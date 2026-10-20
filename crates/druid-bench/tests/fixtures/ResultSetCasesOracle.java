import com.alibaba.druid.pool.DruidDataSource;
import com.alibaba.druid.pool.DruidPooledConnection;
import com.alibaba.druid.pool.DruidPooledResultSet;
import com.alibaba.druid.mock.MockResultSet;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;

/** 从两份冻结原始测试的真实 setup 运行五个 ResultSet 场景。 */
public final class ResultSetCasesOracle {
    private static final String PACKAGE = "com.alibaba.druid.bvt.pool.";
    private ResultSetCasesOracle() {}

    private static void lifecycle(Object test, String name) throws Exception {
        Method method = test.getClass().getDeclaredMethod(name);
        method.setAccessible(true);
        method.invoke(test);
    }

    private static String utf16(String value) {
        if (Objects.isNull(value)) { return "-"; }
        StringBuilder encoded = new StringBuilder();
        for (int index = 0; index < value.length(); index++) {
            encoded.append(String.format("%04x", (int) value.charAt(index)));
        }
        return encoded.toString();
    }

    private static String observe(String[] input) throws Exception {
        Object test = Class.forName(PACKAGE + input[1]).getDeclaredConstructor().newInstance();
        lifecycle(test, "setUp");
        try {
            Field field = test.getClass().getDeclaredField("dataSource");
            field.setAccessible(true);
            DruidDataSource source = (DruidDataSource) field.get(test);
            if (source.getProxyFilters().isEmpty() == Boolean.parseBoolean(input[3])) {
                throw new IllegalArgumentException("original filter setup differs from corpus");
            }
            Connection connection = source.getConnection();
            Connection physical = ((DruidPooledConnection) connection).getConnectionHolder().getConnection();
            PreparedStatement statement = connection.prepareStatement(input[4]);
            statement.setString(1, input[5]);
            DruidPooledResultSet result = (DruidPooledResultSet) statement.executeQuery();
            List<String> trace = new ArrayList<String>();
            Throwable failure = null;
            try {
                switch (input[2]) {
                    case "UNWRAP":
                        if (!Objects.equals(input[6], "java.sql.ResultSet")) { throw new IllegalArgumentException("unwrap target"); }
                        ResultSet raw = result.unwrap(ResultSet.class);
                        trace.add("raw=" + (raw instanceof MockResultSet));
                        trace.add("logical=" + (raw == result));
                        break;
                    case "TYPED_INDEX": result.getObject(Integer.parseInt(input[6]), String.class); break;
                    case "TYPED_LABEL": result.getObject(input[6], String.class); break;
                    case "NAVIGATION":
                        for (String action : input[7].split(",")) {
                            switch (action) {
                                case "next": trace.add(Boolean.toString(result.next())); break;
                                case "previous": trace.add(Boolean.toString(result.previous())); break;
                                case "fetch_count": trace.add(Integer.toString(result.getFetchRowCount())); break;
                                case "row_updated": trace.add(Boolean.toString(result.rowUpdated())); break;
                                default: throw new IllegalArgumentException("unknown navigation action");
                            }
                        }
                        break;
                    default: throw new IllegalArgumentException("unknown scenario");
                }
            } catch (Throwable actual) { failure = actual; }
            // 保留原测试断言：不能让两端同时偏离原始期望而误报 MATCH。
            if (Objects.equals(input[2], "UNWRAP") && (!Objects.isNull(failure) || !trace.contains("raw=true"))) {
                throw new AssertionError("original testWrap requires native MockResultSet");
            }
            if (input[2].startsWith("TYPED_") && !(failure instanceof java.sql.SQLFeatureNotSupportedException)) {
                throw new AssertionError("original typed getObject requires SQLFeatureNotSupportedException");
            }
            if (Objects.equals(input[2], "NAVIGATION") && (!Objects.isNull(failure)
                || !Objects.equals(String.join(",", trace), "true,false,1,true,false,1,true,false,1,false"))) {
                throw new AssertionError("original rowCount navigation/count assertions changed");
            }
            SQLException sql = failure instanceof SQLException ? (SQLException) failure : null;
            String outcome = Objects.isNull(failure) ? "RETURN" : "THROW";
            String errorClass = Objects.isNull(failure) ? "-" : failure.getClass().getName();
            String code = Objects.isNull(sql) ? "-" : Integer.toString(sql.getErrorCode());
            String state = Objects.isNull(sql) ? "-" : utf16(sql.getSQLState());
            String message = Objects.isNull(failure) ? "-" : utf16(failure.getMessage());
            long errors = source.getErrorCount();
            int activeDuring = source.getActiveCount();
            int idleDuring = source.getPoolingCount();
            result.close();
            connection.close();
            boolean cursorClosed = result.isClosed();
            boolean statementClosed = statement.isClosed();
            int activeAfter = source.getActiveCount();
            int idleAfter = source.getPoolingCount();
            Connection next = source.getConnection();
            boolean reused = ((DruidPooledConnection) next).getConnectionHolder().getConnection() == physical;
            next.close();
            return input[0] + "\t" + (trace.isEmpty() ? "-" : String.join(",", trace))
                + "\t" + outcome + "\t" + errorClass + "\t" + code + "\t" + state + "\t" + message
                + "\t" + errors + "\t" + activeDuring + "\t" + idleDuring + "\t" + cursorClosed
                + "\t" + statementClosed + "\t" + activeAfter + "\t" + idleAfter + "\t" + reused;
        } finally { lifecycle(test, "tearDown"); }
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2 || !Objects.equals(com.alibaba.druid.VERSION.getVersionNumber(), "1.2.28")) {
            throw new IllegalArgumentException("frozen runtime and two paths required");
        }
        byte[] bytes = Files.readAllBytes(Paths.get(args[0]));
        StringBuilder digest = new StringBuilder();
        for (byte value : MessageDigest.getInstance("SHA-256").digest(bytes)) {
            digest.append(String.format("%02x", value & 0xff));
        }
        if (!Objects.equals(digest.toString(), "991d7898de149880a7dc4a4e4dc767d1e5cfdc73f7b2444dbedc7561b8037de6")) {
            throw new IllegalArgumentException("frozen corpus drift");
        }
        Set<String> expected = new HashSet<String>();
        for (String name : new String[] {"DruidPooledResultSetTest", "DruidPooledResultSetTest2"}) {
            for (Method method : Class.forName(PACKAGE + name).getDeclaredMethods()) {
                if (method.getName().startsWith("test")) { expected.add(name + "." + method.getName()); }
            }
        }
        Set<String> seen = new HashSet<String>();
        List<String> records = new ArrayList<String>();
        records.add("case_id\ttrace\toutcome\terror_class\terror_code\tsql_state_utf16\tmessage_utf16\terrors_after\tactive_during\tidle_during\tcursor_closed_after\tstatement_closed_after\tactive_after\tidle_after\treused");
        for (String line : Files.readAllLines(Paths.get(args[0]), StandardCharsets.UTF_8)) {
            if (line.startsWith("#")) { continue; }
            String[] input = line.split("\t", -1);
            if (input.length != 8 || !seen.add(input[0])) { throw new IllegalArgumentException("invalid or duplicate input"); }
            records.add(observe(input));
        }
        if (expected.size() != 5 || !expected.equals(seen)) { throw new IllegalArgumentException("original denominator differs"); }
        Files.write(Paths.get(args[1]), records, StandardCharsets.UTF_8);
    }
}
