import com.alibaba.druid.VERSION;
import com.alibaba.druid.bvt.filter.wall.WallMultiStatementTest;
import com.alibaba.druid.wall.WallUtils;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/** 对应冻结 WallMultiStatementTest 的全部两例；先执行原断言，再采集同输入结果。 */
public final class WallMultiCasesOracle {
    private WallMultiCasesOracle() {}

    public static void main(String[] args) throws Exception {
        if (!"1.2.28".equals(VERSION.getVersionNumber())) {
            throw new IllegalArgumentException("Druid version drift");
        }
        byte[] bytes = Files.readAllBytes(Paths.get(args[0]));
        StringBuilder digest = new StringBuilder();
        for (byte value : MessageDigest.getInstance("SHA-256").digest(bytes)) {
            digest.append(String.format("%02x", value & 255));
        }
        if (!"682ab4352eafee9f3f667c3a09d8e8133561e78acc79b38b773bec65f659b8b9".equals(digest.toString())) {
            throw new IllegalArgumentException("corpus drift");
        }
        List<String> lines = Files.readAllLines(Paths.get(args[0]), StandardCharsets.UTF_8);
        Set<String> original = new HashSet<>();
        for (Method method : WallMultiStatementTest.class.getDeclaredMethods()) {
            if (method.getName().startsWith("test") && method.getParameterCount() == 0) {
                original.add(method.getName());
            }
        }
        if (!original.equals(new HashSet<>(Arrays.asList("testOracle", "testMySql")))
                || lines.size() != 3 || !"case_id\tdialect\tsql".equals(lines.get(0))) {
            throw new IllegalArgumentException("original denominator drift");
        }
        StringBuilder results = new StringBuilder("case_id\tvalid\n");
        for (String line : lines.subList(1, lines.size())) {
            String[] input = line.split("\t", -1);
            if (input.length != 3 || !original.remove(input[0])) {
                throw new IllegalArgumentException("unknown or duplicate input");
            }
            WallMultiStatementTest test = new WallMultiStatementTest();
            Field field = WallMultiStatementTest.class.getDeclaredField("sql");
            field.setAccessible(true);
            if (!input[2].equals(field.get(test))) {
                throw new IllegalArgumentException("original SQL differs");
            }
            WallMultiStatementTest.class.getMethod(input[0]).invoke(test);
            boolean valid;
            if ("testOracle".equals(input[0]) && "oracle".equals(input[1])) {
                valid = WallUtils.isValidateOracle(input[2]);
            } else if ("testMySql".equals(input[0]) && "mysql".equals(input[1])) {
                valid = WallUtils.isValidateMySql(input[2]);
            } else {
                throw new IllegalArgumentException("method/dialect mapping drift");
            }
            results.append(input[0]).append('\t').append(valid).append('\n');
        }
        Files.write(Paths.get(args[1]), results.toString().getBytes(StandardCharsets.UTF_8));
    }
}
