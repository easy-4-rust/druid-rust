import com.alibaba.druid.VERSION;
import com.alibaba.druid.bvt.filter.wall.WallFilterTest;
import com.alibaba.druid.wall.WallFilter;
import com.alibaba.druid.wall.WallTableStat;
import com.alibaba.druid.pool.DruidDataSource;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.Statement;
import java.sql.ResultSet;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/** 对应 Java WallFilterTest：先运行原断言，再用原 setUp 采集真实 H2 表统计检查点。 */
public final class WallTableCaseOracle {
    private WallTableCaseOracle() {}
    private static void lifecycle(WallFilterTest test, String name) throws Exception {
        Method method = WallFilterTest.class.getDeclaredMethod(name);
        method.setAccessible(true);
        method.invoke(test);
    }

    public static void main(String[] args) throws Exception {
        if (!"1.2.28".equals(VERSION.getVersionNumber())) { throw new IllegalArgumentException("version drift"); }
        byte[] bytes = Files.readAllBytes(Paths.get(args[0]));
        StringBuilder digest = new StringBuilder();
        for (byte value : MessageDigest.getInstance("SHA-256").digest(bytes)) { digest.append(String.format("%02x", value & 255)); }
        if (!"0494e11cb12bc5bc9e139fbd861fc5fcdf7ce8fd09008fc192f3dbbb03367dbf".equals(digest.toString())) { throw new IllegalArgumentException("corpus drift"); }
        List<String> lines = Files.readAllLines(Paths.get(args[0]), StandardCharsets.UTF_8);
        Set<String> methods = new HashSet<>();
        for (Method method : WallFilterTest.class.getDeclaredMethods()) {
            if (method.getName().startsWith("test") && method.getParameterCount() == 0) { methods.add(method.getName()); }
        }
        if (!methods.equals(new HashSet<>(Arrays.asList("test_wallFilter"))) || lines.size() != 21
                || !"step\toperation\tsql\tparameters\tcheckpoint".equals(lines.get(0))) { throw new IllegalArgumentException("denominator/input drift"); }
        WallFilterTest original = new WallFilterTest();
        lifecycle(original, "setUp");
        try { original.test_wallFilter(); } finally { lifecycle(original, "tearDown"); }
        WallFilterTest observation = new WallFilterTest();
        lifecycle(observation, "setUp");
        try {
            Field field = WallFilterTest.class.getDeclaredField("dataSource");
            field.setAccessible(true);
            DruidDataSource source = (DruidDataSource) field.get(observation);
            Field wallField = WallFilterTest.class.getDeclaredField("wallFilter");
            wallField.setAccessible(true);
            WallFilter wall = (WallFilter) wallField.get(observation);
            StringBuilder checkpoints = new StringBuilder();
            Connection insertsConnection = null;
            Statement inserts = null;
            for (int step = 0; step < 20; step++) {
                String[] input = lines.get(step + 1).split("\t", -1);
                if (input.length != 5 || !Integer.toString(step).equals(input[0])) { throw new IllegalArgumentException("operation drift"); }
                if (step == 1) { insertsConnection = source.getConnection(); inserts = insertsConnection.createStatement(); }
                if (step >= 1 && step <= 10) {
                    inserts.execute(input[2]);
                    if (step == 10) { inserts.close(); insertsConnection.close(); }
                    continue;
                }
                Connection connection = source.getConnection();
                if ("statement".equals(input[1])) {
                    Statement statement = connection.createStatement();
                    statement.execute(input[2]);
                    statement.close();
                } else {
                    PreparedStatement statement = connection.prepareStatement(input[2]);
                    if ("query".equals(input[1])) {
                        ResultSet result = statement.executeQuery();
                        while (result.next()) { /* 运行原始逐行消费路径。 */ }
                        result.close();
                    } else if ("prepared".equals(input[1]) || "batch".equals(input[1])) {
                        String[] values = input[3].split(",");
                        for (int index = 0; index < values.length; index++) {
                            statement.setInt("batch".equals(input[1]) ? 1 : index + 1, Integer.parseInt(values[index]));
                            if ("batch".equals(input[1])) { statement.addBatch(); }
                        }
                        if ("batch".equals(input[1])) { statement.executeBatch(); } else { statement.execute(); }
                    } else { throw new IllegalArgumentException("unknown operation"); }
                    statement.close();
                }
                connection.close();
                WallTableStat stat = wall.getProvider().getTableStat("t");
                long count;
                switch (input[4]) {
                    case "create_count": count = stat.getCreateCount(); break;
                    case "fetch_row_count": count = stat.getFetchRowCount(); break;
                    case "delete_data_count": count = stat.getDeleteDataCount(); break;
                    case "update_data_count": count = stat.getUpdateDataCount(); break;
                    case "truncate_count": count = stat.getTruncateCount(); break;
                    case "drop_count": count = stat.getDropCount(); break;
                    default: throw new IllegalArgumentException("unknown checkpoint");
                }
                if (checkpoints.length() > 0) { checkpoints.append(','); }
                checkpoints.append(count);
            }
            Files.write(Paths.get(args[1]), ("case_id\tcheckpoints\ntest_wallFilter\t" + checkpoints + "\n").getBytes(StandardCharsets.UTF_8));
        } finally { lifecycle(observation, "tearDown"); }
    }
}
