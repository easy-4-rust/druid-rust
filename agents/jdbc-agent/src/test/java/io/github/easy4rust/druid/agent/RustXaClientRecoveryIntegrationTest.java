package io.github.easy4rust.druid.agent;

import org.junit.jupiter.api.Assumptions;
import org.junit.jupiter.api.Test;
import org.postgresql.xa.PGXADataSource;
import org.testcontainers.postgresql.PostgreSQLContainer;

import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.ResultSet;
import java.sql.Statement;
import java.time.Duration;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** Rust RDBC -> JDBC Agent -> PostgreSQL XA recovery end-to-end proof. */
class RustXaClientRecoveryIntegrationTest {

    @Test
    void rustClientRecoversPreparedBranchesAcrossSessions() throws Exception {
        Assumptions.assumeTrue(
                Boolean.getBoolean("druid.rustXaIntegration"),
                "set -Ddruid.rustXaIntegration=true to run the cross-language PostgreSQL proof");

        try (PostgreSQLContainer postgres = new PostgreSQLContainer("postgres:18.6")
                .withDatabaseName("xa_recovery")
                .withUsername("xa_user")
                .withPassword("xa_password")
                .withCommand("postgres", "-c", "max_prepared_transactions=10")) {
            postgres.start();
            Path projectDirectory = Path.of(System.getProperty("user.dir")).toAbsolutePath();
            Path repository = projectDirectory.getParent().getParent();
            Path agentJar = repository.resolve("agents/jdbc-agent/target/druid-jdbc-agent.jar");
            assertTrue(Files.isRegularFile(agentJar), "build the shaded JDBC Agent before running");
            Path driverJar = Path.of(PGXADataSource.class.getProtectionDomain()
                    .getCodeSource().getLocation().toURI()).toAbsolutePath();
            assertTrue(Files.isRegularFile(driverJar), "PostgreSQL JDBC test jar must be available");
            Path java = Path.of(System.getProperty("java.home"), "bin", "java.exe");
            if (!Files.isRegularFile(java)) {
                java = Path.of(System.getProperty("java.home"), "bin", "java");
            }

            Path output = repository.resolve("agents/jdbc-agent/target/rust-xa-client-recovery.log");
            ProcessBuilder builder = new ProcessBuilder(
                    "cargo", "test", "-p", "druid-wrapper", "--features", "jdbc-agent",
                    "--test", "jdbc_agent_xa_external_consumer", "--locked", "--offline",
                    "rust_client_recovers_prepared_branches_across_sessions", "--",
                    "--ignored", "--exact", "--nocapture");
            builder.directory(repository.toFile());
            builder.redirectErrorStream(true);
            builder.redirectOutput(output.toFile());
            builder.environment().put("DRUID_JAVA_EXECUTABLE", java.toString());
            builder.environment().put("DRUID_JDBC_AGENT_JAR", agentJar.toString());
            builder.environment().put("DRUID_JDBC_DRIVER_JAR", driverJar.toString());
            builder.environment().put("DRUID_XA_PG_HOST", postgres.getHost());
            builder.environment().put("DRUID_XA_PG_PORT", Integer.toString(postgres.getFirstMappedPort()));
            builder.environment().put("DRUID_XA_PG_DATABASE", postgres.getDatabaseName());
            builder.environment().put("DRUID_XA_PG_USER", postgres.getUsername());
            builder.environment().put("DRUID_XA_PG_PASSWORD", postgres.getPassword());

            Process cargo = builder.start();
            boolean completed = cargo.waitFor(Duration.ofMinutes(10).toMillis(), TimeUnit.MILLISECONDS);
            if (!completed) {
                cargo.destroyForcibly();
                cargo.waitFor(30, TimeUnit.SECONDS);
            }
            String log = Files.exists(output) ? Files.readString(output) : "<no cargo output>";
            assertTrue(completed, "Rust XA test exceeded 10 minutes:\n" + log);
            assertEquals(0, cargo.exitValue(), "Rust XA integration test failed:\n" + log);
            assertTrue(log.contains("1 passed"), "Cargo must run exactly one real test:\n" + log);
            assertFalse(log.contains("0 tests"), "Cargo must not silently match zero tests:\n" + log);

            try (Connection independent = DriverManager.getConnection(
                    postgres.getJdbcUrl(), postgres.getUsername(), postgres.getPassword());
                 Statement statement = independent.createStatement();
                 ResultSet rows = statement.executeQuery(
                         "SELECT id, payload FROM druid_rust_xa_recovery ORDER BY id")) {
                assertTrue(rows.next(), "phase-two commit must persist its row");
                assertEquals(1, rows.getInt(1));
                assertEquals("commit-after-recovery", rows.getString(2));
                assertFalse(rows.next(), "the recovered rollback branch must leave no row");
            }
        }
    }
}
