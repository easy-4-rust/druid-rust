import com.alibaba.druid.bvt.pool.DruidPooledStatementTest;
import com.alibaba.druid.pool.DruidDataSource;
import com.alibaba.druid.pool.DruidPooledConnection;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.sql.Connection;
import java.sql.SQLException;
import java.sql.Statement;
import java.security.MessageDigest;
import java.util.HashSet;
import java.util.Set;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/** 对冻结的37个原始语句调用逐项观察异常与池生命周期，不预置预期结果。 */
public final class StatementErrorCasesOracle extends DruidPooledStatementTest {
    private StatementErrorCasesOracle() {}

    private static String utf16(String value) {
        if (Objects.isNull(value)) { return "-"; }
        StringBuilder encoded = new StringBuilder();
        for (int index = 0; index < value.length(); index++) {
            encoded.append(String.format("%04x", (int) value.charAt(index)));
        }
        return encoded.toString();
    }

    private static void invoke(Statement stmt, String[] input) throws Throwable {
        String method = input[1];
        String shape = input[2];
        String value = input[3];
        Class<?>[] types;
        Object[] values;
        switch (shape) {
            case "NONE": types = new Class<?>[0]; values = new Object[0]; break;
            case "TEXT":
                types = new Class<?>[] {String.class};
                values = new Object[] {Objects.equals(value, "-") ? null : value}; break;
            case "INT":
                types = new Class<?>[] {int.class}; values = new Object[] {Integer.valueOf(value)}; break;
            case "BOOLEAN":
                if (!Objects.equals(value, "true") && !Objects.equals(value, "false")) {
                    throw new IllegalArgumentException("invalid boolean");
                }
                types = new Class<?>[] {boolean.class}; values = new Object[] {Boolean.valueOf(value)}; break;
            case "SQL_INT":
                types = new Class<?>[] {String.class, int.class};
                values = new Object[] {value, Integer.valueOf(input[4])}; break;
            case "SQL_INT_ARRAY":
                if (!Objects.equals(input[4], "[]")) { throw new IllegalArgumentException("invalid index array"); }
                types = new Class<?>[] {String.class, int[].class}; values = new Object[] {value, new int[0]}; break;
            case "SQL_STRING_ARRAY":
                if (!Objects.equals(input[4], "[]")) { throw new IllegalArgumentException("invalid name array"); }
                types = new Class<?>[] {String.class, String[].class}; values = new Object[] {value, new String[0]}; break;
            default: throw new IllegalArgumentException("unknown argument shape: " + shape);
        }
        try { Statement.class.getMethod(method, types).invoke(stmt, values); }
        catch (java.lang.reflect.InvocationTargetException error) { throw error.getCause(); }
    }

    private String observe(String[] input) throws Exception {
        String id = input[0];
        super.setUp();
        try {
            Field field = DruidPooledStatementTest.class.getDeclaredField("dataSource");
            field.setAccessible(true);
            DruidDataSource source = (DruidDataSource) field.get(this);
            Connection connection = source.getConnection();
            Connection physical = ((DruidPooledConnection) connection).getConnectionHolder().getConnection();
            Statement statement = connection.createStatement();
            long errorsBefore = source.getErrorCount();
            int activeBefore = source.getActiveCount();
            int idleBefore = source.getPoolingCount();
            Throwable failure = null;
            try { invoke(statement, input); } catch (Throwable actual) { failure = actual; }
            String outcome = Objects.isNull(failure) ? "RETURN" : "THROW";
            String errorClass = Objects.isNull(failure) ? "-" : failure.getClass().getName();
            SQLException sql = failure instanceof SQLException ? (SQLException) failure : null;
            String code = Objects.isNull(sql) ? "-" : Integer.toString(sql.getErrorCode());
            String state = Objects.isNull(sql) ? "-" : utf16(sql.getSQLState());
            String message = Objects.isNull(failure) ? "-" : utf16(failure.getMessage());
            long errorsAfter = source.getErrorCount();
            int activeDuring = source.getActiveCount();
            int idleDuring = source.getPoolingCount();
            statement.close();
            connection.close();
            int activeAfter = source.getActiveCount();
            int idleAfter = source.getPoolingCount();
            Connection next = source.getConnection();
            boolean reused = ((DruidPooledConnection) next).getConnectionHolder().getConnection() == physical;
            next.close();
            return id + "\t" + outcome + "\t" + errorClass + "\t" + code + "\t" + state
                + "\t" + message + "\t" + errorsBefore + "\t" + activeBefore + "\t" + idleBefore
                + "\t" + errorsAfter + "\t" + activeDuring + "\t" + idleDuring
                + "\t" + activeAfter + "\t" + idleAfter + "\t" + reused;
        } finally { super.tearDown(); }
    }

    public static void main(String[] args) throws Exception {
        if (!Objects.equals(com.alibaba.druid.VERSION.getVersionNumber(), "1.2.28")) {
            throw new IllegalArgumentException("expected frozen Druid 1.2.28 runtime");
        }
        if (args.length != 2) { throw new IllegalArgumentException("expected corpus and output paths"); }
        byte[] corpus = Files.readAllBytes(Paths.get(args[0]));
        StringBuilder digest = new StringBuilder();
        for (byte value : MessageDigest.getInstance("SHA-256").digest(corpus)) {
            digest.append(String.format("%02x", value & 0xff));
        }
        if (!Objects.equals(digest.toString(), "2fa81686348ffe8abe951181735fb1c3916d9df0038267fb87177d9ea5e1f04d")) {
            throw new IllegalArgumentException("frozen corpus digest changed");
        }
        Set<String> seen = new HashSet<String>();
        List<String> results = new ArrayList<String>();
        results.add("case_id\toutcome\terror_class\terror_code\tsql_state_utf16\tmessage_utf16\terrors_before\tactive_before\tidle_before\terrors_after\tactive_during\tidle_during\tactive_after\tidle_after\treused");
        for (String row : Files.readAllLines(Paths.get(args[0]), StandardCharsets.UTF_8)) {
            if (row.startsWith("#")) { continue; }
            String[] fields = row.split("\t", -1);
            if (fields.length != 6) { throw new IllegalArgumentException("invalid case row"); }
            if (!seen.add(fields[0])) { throw new IllegalArgumentException("duplicate case"); }
            results.add(new StatementErrorCasesOracle().observe(fields));
        }
        Set<String> expected = new HashSet<String>();
        for (java.lang.reflect.Method method : DruidPooledStatementTest.class.getDeclaredMethods()) {
            if (method.getName().startsWith("test_")) { expected.add(method.getName()); }
        }
        if (expected.size() != 37 || !expected.equals(seen)) {
            throw new IllegalArgumentException("frozen test denominator changed");
        }
        Files.write(Paths.get(args[1]), results, StandardCharsets.UTF_8);
    }
}
