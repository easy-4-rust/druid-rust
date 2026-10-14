package io.github.easy4rust.druid.agent;

import org.junit.jupiter.api.Test;
import org.testcontainers.DockerClientFactory;
import org.postgresql.xa.PGXADataSource;
import org.testcontainers.postgresql.PostgreSQLContainer;

import javax.sql.XAConnection;
import javax.transaction.xa.XAResource;
import javax.transaction.xa.Xid;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.Statement;
import java.util.Arrays;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

/** 使用真实 PostgreSQL 验证 XA 分支跨 XAConnection 关闭后的恢复语义。 */
class XaRecoveryIntegrationTest {

    @Test
    void postgresRecoversPreparedBranchAfterXaConnectionCloses() throws Exception {
        assumeTrue(DockerClientFactory.instance().isDockerAvailable(), "Docker is unavailable");
        PostgreSQLContainer postgres = new PostgreSQLContainer("postgres:18.6")
                .withDatabaseName("xa_recovery")
                .withUsername("xa_user")
                .withPassword("xa_password")
                .withCommand("postgres", "-c", "max_prepared_transactions=10");
        try {
            postgres.start();
            PGXADataSource dataSource = new PGXADataSource();
            dataSource.setServerNames(new String[] {postgres.getHost()});
            dataSource.setPortNumbers(new int[] {postgres.getMappedPort(5432)});
            dataSource.setDatabaseName(postgres.getDatabaseName());
            dataSource.setUser(postgres.getUsername());
            dataSource.setPassword(postgres.getPassword());
            try (Connection setup = DriverManager.getConnection(
                    postgres.getJdbcUrl(), postgres.getUsername(), postgres.getPassword());
                 Statement statement = setup.createStatement()) {
                statement.execute("CREATE TABLE recovered_xa(id INT PRIMARY KEY, payload VARCHAR(32))");
            }

            Xid xid = new TestXid(0x44525549, new byte[] {1, 2, 3}, new byte[] {4, 5});
            XAConnection owner = dataSource.getXAConnection();
            try {
                XAResource resource = owner.getXAResource();
                Connection logical = owner.getConnection();
                resource.start(xid, XAResource.TMNOFLAGS);
                try (PreparedStatement statement = logical.prepareStatement(
                        "INSERT INTO recovered_xa(id, payload) VALUES (?, ?)")) {
                    statement.setInt(1, 1);
                    statement.setString(2, "prepared");
                    assertEquals(1, statement.executeUpdate());
                }
                resource.end(xid, XAResource.TMSUCCESS);
                assertEquals(XAResource.XA_OK, resource.prepare(xid));
                logical.close();
            } finally {
                owner.close();
            }

            XAConnection recoveryOwner = dataSource.getXAConnection();
            try {
                XAResource recovery = recoveryOwner.getXAResource();
                Xid[] recovered = recovery.recover(
                        XAResource.TMSTARTRSCAN | XAResource.TMENDRSCAN);
                assertTrue(Arrays.stream(recovered).anyMatch(candidate -> sameXid(xid, candidate)));
                recovery.commit(xid, false);
            } finally {
                recoveryOwner.close();
            }

            try (Connection verification = DriverManager.getConnection(
                    postgres.getJdbcUrl(), postgres.getUsername(), postgres.getPassword());
                 Statement statement = verification.createStatement();
                 ResultSet rows = statement.executeQuery(
                         "SELECT COUNT(*) FROM recovered_xa WHERE id = 1 AND payload = 'prepared'")) {
                assertTrue(rows.next());
                assertEquals(1, rows.getInt(1));
            }
        } finally {
            postgres.stop();
        }
    }

    private boolean sameXid(Xid expected, Xid actual) {
        return expected.getFormatId() == actual.getFormatId()
                && Arrays.equals(expected.getGlobalTransactionId(), actual.getGlobalTransactionId())
                && Arrays.equals(expected.getBranchQualifier(), actual.getBranchQualifier());
    }

    private static final class TestXid implements Xid {
        private final int formatId;
        private final byte[] globalTransactionId;
        private final byte[] branchQualifier;

        private TestXid(int formatId, byte[] globalTransactionId, byte[] branchQualifier) {
            this.formatId = formatId;
            this.globalTransactionId = globalTransactionId.clone();
            this.branchQualifier = branchQualifier.clone();
        }

        @Override
        public int getFormatId() {
            return formatId;
        }

        @Override
        public byte[] getGlobalTransactionId() {
            return globalTransactionId.clone();
        }

        @Override
        public byte[] getBranchQualifier() {
            return branchQualifier.clone();
        }
    }
}
