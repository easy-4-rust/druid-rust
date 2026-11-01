import java.lang.reflect.Method;
import java.lang.reflect.InvocationTargetException;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/** 只调用当前原始测试方法；显式复用 PoolTestCase 生命周期，避免缺少 @Test 时零发现。 */
public final class SourceSorterTests {
    public static void main(String[] names) throws Exception {
        int total = 0;
        for (String name : names) {
            Class<?> type = Class.forName("com.alibaba.druid.bvt.pool.vendor." + name);
            List<Method> tests = new ArrayList<>();
            for (Method method : type.getDeclaredMethods()) {
                if (method.getName().startsWith("test") && method.getParameterCount() == 0) {
                    tests.add(method);
                }
            }
            tests.sort(Comparator.comparing(Method::getName));
            if (tests.isEmpty()) { throw new AssertionError("zero tests: " + name); }
            for (Method test : tests) {
                Object instance = type.getDeclaredConstructor().newInstance();
                Method setup = type.getSuperclass().getDeclaredMethod("setUp");
                Method teardown = type.getSuperclass().getDeclaredMethod("tearDown");
                setup.setAccessible(true);
                teardown.setAccessible(true);
                setup.invoke(instance);
                try {
                    test.invoke(instance);
                    System.out.println("PASS\t" + name + "#" + test.getName());
                    total++;
                } catch (InvocationTargetException error) {
                    throw new AssertionError(name + "#" + test.getName(), error.getCause());
                } finally {
                    teardown.invoke(instance);
                }
            }
        }
        if (total != 18) { throw new AssertionError("denominator drift: " + total); }
        System.out.println("TOTAL\t" + total);
    }
}
