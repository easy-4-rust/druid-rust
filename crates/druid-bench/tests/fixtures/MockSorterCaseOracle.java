import com.alibaba.druid.VERSION;
import com.alibaba.druid.bvt.pool.ExceptionSorterTest;
import com.alibaba.druid.mock.MockConnection;
import com.alibaba.druid.pool.DruidDataSource;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.SQLException;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;

/** 对应 Java ExceptionSorterTest：先运行原断言，再用原 setUp 采集清理结果。 */
public final class MockSorterCaseOracle {
    private MockSorterCaseOracle() {}
    private static void lifecycle(ExceptionSorterTest test, String name) throws Exception {
        Method method = ExceptionSorterTest.class.getDeclaredMethod(name);
        method.setAccessible(true);
        method.invoke(test);
    }
    private static String nullable(String value) { return Objects.isNull(value) ? "-" : value; }
    public static void main(String[] args) throws Exception {
        if (!"1.2.28".equals(VERSION.getVersionNumber())) { throw new IllegalArgumentException("version drift"); }
        byte[] bytes = Files.readAllBytes(Paths.get(args[0]));
        StringBuilder digest = new StringBuilder();
        for (byte value : MessageDigest.getInstance("SHA-256").digest(bytes)) { digest.append(String.format("%02x", value & 255)); }
        if (!"69faf3454701669a2dc6a2095285a73684c8298364ed1615b05aedf09eefb952".equals(digest.toString())) { throw new IllegalArgumentException("corpus drift"); }
        List<String> lines = Files.readAllLines(Paths.get(args[0]), StandardCharsets.UTF_8);
        Set<String> methods = new HashSet<>();
        for (Method method : ExceptionSorterTest.class.getDeclaredMethods()) {
            if (method.getName().startsWith("test") && method.getParameterCount() == 0) { methods.add(method.getName()); }
        }
        if (!methods.equals(new HashSet<>(Arrays.asList("test_exceptionSorter"))) || lines.size() != 2
                || !"test_exceptionSorter\tmock\tmysql\tselect 1".equals(lines.get(1))) { throw new IllegalArgumentException("denominator/input drift"); }
        ExceptionSorterTest original = new ExceptionSorterTest();
        lifecycle(original, "setUp");
        try { original.test_exceptionSorter(); } finally { lifecycle(original, "tearDown"); }
        ExceptionSorterTest observation = new ExceptionSorterTest();
        lifecycle(observation, "setUp");
        try {
            Field field = ExceptionSorterTest.class.getDeclaredField("dataSource");
            field.setAccessible(true);
            DruidDataSource source = (DruidDataSource) field.get(observation);
            String sorter = source.getExceptionSorter().getClass().getSimpleName();
            Connection connection = source.getConnection();
            MockConnection raw = connection.unwrap(MockConnection.class);
            PreparedStatement statement = connection.prepareStatement("select 1");
            statement.execute();
            raw.close();
            SQLException failure = null;
            try { statement.close(); } catch (SQLException error) { failure = error; }
            if (Objects.isNull(failure)) { throw new AssertionError("missing cleanup failure"); }
            connection.close();
            String header = "case_id\tsorter\terror_class\terror_code\tsql_state\tmessage\tdiscard_count\tactive_count\tidle_count\n";
            String row = "test_exceptionSorter\t" + sorter + "\t" + failure.getClass().getName() + "\t" + failure.getErrorCode()
                    + "\t" + nullable(failure.getSQLState()) + "\t" + nullable(failure.getMessage()) + "\t" + source.getDiscardCount()
                    + "\t" + source.getActiveCount() + "\t" + source.getPoolingCount() + "\n";
            Files.write(Paths.get(args[1]), (header + row).getBytes(StandardCharsets.UTF_8));
        } finally { lifecycle(observation, "tearDown"); }
    }
}
