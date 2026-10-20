import com.alibaba.druid.VERSION;
import com.alibaba.druid.bvt.pool.DruidPooledPreparedStatementTest;
import com.alibaba.druid.pool.DruidPooledPreparedStatement;
import com.alibaba.druid.pool.DruidDataSource;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.sql.Connection;
import java.sql.ResultSet;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;

/** 对应 Java DruidPooledPreparedStatementTest：先运行原断言，再用原 setUp 采集清理结果。 */
public final class PreparedCacheCaseOracle {
    private PreparedCacheCaseOracle() {}
    private static void lifecycle(DruidPooledPreparedStatementTest test, String name) throws Exception {
        Method method = DruidPooledPreparedStatementTest.class.getDeclaredMethod(name);
        method.setAccessible(true);
        method.invoke(test);
    }
    private static String properties(DruidPooledPreparedStatement statement) throws Exception {
        return statement.getFetchSize() + "," + statement.getFetchDirection() + "," + statement.getMaxRows() + "," + statement.getMaxFieldSize() + "," + statement.getQueryTimeout();
    }
    public static void main(String[] args) throws Exception {
        if (!"1.2.28".equals(VERSION.getVersionNumber())) { throw new IllegalArgumentException("version drift"); }
        byte[] bytes = Files.readAllBytes(Paths.get(args[0]));
        StringBuilder digest = new StringBuilder();
        for (byte value : MessageDigest.getInstance("SHA-256").digest(bytes)) { digest.append(String.format("%02x", value & 255)); }
        if (!"7681fa4dac8ffdb02343fb72a5868f39a5f5df3b7c66f5590f7a9d27c26f2d10".equals(digest.toString())) { throw new IllegalArgumentException("corpus drift"); }
        List<String> lines = Files.readAllLines(Paths.get(args[0]), StandardCharsets.UTF_8);
        Set<String> methods = new HashSet<>();
        for (Method method : DruidPooledPreparedStatementTest.class.getDeclaredMethods()) {
            if (method.getName().startsWith("test") && method.getParameterCount() == 0) { methods.add(method.getName()); }
        }
        if (!methods.equals(new HashSet<>(Arrays.asList("test_hitCount"))) || lines.size() != 2
                || !"test_hitCount\tselect ?\txx\t1,2,3,4,5".equals(lines.get(1))) { throw new IllegalArgumentException("denominator/input drift"); }
        DruidPooledPreparedStatementTest original = new DruidPooledPreparedStatementTest();
        lifecycle(original, "setUp");
        try { original.test_hitCount(); } finally { lifecycle(original, "tearDown"); }
        DruidPooledPreparedStatementTest observation = new DruidPooledPreparedStatementTest();
        lifecycle(observation, "setUp");
        try {
            Field field = DruidPooledPreparedStatementTest.class.getDeclaredField("dataSource");
            field.setAccessible(true);
            DruidDataSource source = (DruidDataSource) field.get(observation);
            String[] initial = new String[2];
            String[] traversal = new String[2];
            int[] hit = new int[2];
            String mutated = "";
            String sql = "";
            boolean key = false;
            for (int round = 0; round < 2; round++) {
                Connection connection = source.getConnection();
                DruidPooledPreparedStatement statement = (DruidPooledPreparedStatement) connection.prepareStatement("select ?");
                initial[round] = properties(statement);
                if (round == 0) {
                    sql = statement.getSql();
                    statement.setFetchSize(1);
                    statement.setFetchDirection(2);
                    statement.setMaxRows(3);
                    statement.setMaxFieldSize(4);
                    statement.setQueryTimeout(5);
                    mutated = properties(statement);
                }
                statement.setString(1, "xx");
                ResultSet result = statement.executeQuery();
                traversal[round] = result.next() + "," + result.next();
                result.close();
                connection.close();
                hit[round] = statement.getHitCount();
                if (round == 1) { key = Objects.nonNull(statement.getKey()); }
            }
            String header = "case_id\tbefore_first\tmutated\tbefore_second\tfirst_rows\tsecond_rows\tsql\tfirst_hit\tsecond_hit\tkey_present\tpool_hits\tactive_count\tidle_count\n";
            String row = "test_hitCount\t" + initial[0] + "\t" + mutated + "\t" + initial[1] + "\t" + traversal[0] + "\t" + traversal[1]
                    + "\t" + sql + "\t" + hit[0] + "\t" + hit[1] + "\t" + key + "\t" + source.getCachedPreparedStatementHitCount()
                    + "\t" + source.getActiveCount() + "\t" + source.getPoolingCount() + "\n";
            Files.write(Paths.get(args[1]), (header + row).getBytes(StandardCharsets.UTF_8));
        } finally { lifecycle(observation, "tearDown"); }
    }
}
