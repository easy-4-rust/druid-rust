import java.lang.reflect.*;
import java.lang.invoke.MethodHandles;
import java.sql.JDBCType;
import java.sql.SQLType;
import java.sql.SQLException;
import java.security.MessageDigest;
import java.io.InputStream;
import java.util.*;

public final class Jdbc42DefaultOracle {
    private static String classHash(Class<?> type) throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (InputStream input = type.getResourceAsStream("/" + type.getName().replace('.', '/') + ".class")) {
            if (input == null) throw new AssertionError("missing class resource");
            byte[] buffer = new byte[4096]; int count;
            while ((count = input.read(buffer)) != -1) digest.update(buffer, 0, count);
        }
        StringBuilder result = new StringBuilder();
        for (byte value : digest.digest()) result.append(String.format("%02x", value & 255));
        return result.toString();
    }
    private static Object argument(Class<?> type) {
        if (type == int.class) return 1;
        if (type == long.class) return 0L;
        if (type == boolean.class) return false;
        if (type == String.class) return "probe";
        if (type == int[].class) return new int[0];
        if (type == String[].class) return new String[0];
        if (type == SQLType.class) return JDBCType.INTEGER;
        if (type == Class.class) return Object.class;
        if (type == Object.class) return null;
        throw new AssertionError("unhandled parameter " + type);
    }
    public static void main(String[] names) throws Exception {
        Constructor<MethodHandles.Lookup> constructor = MethodHandles.Lookup.class.getDeclaredConstructor(Class.class, int.class);
        constructor.setAccessible(true);
        int total = 0;
        for (String name : names) {
            Class<?> owner = Class.forName(name, false, null);
            if (owner.getClassLoader() != null) throw new AssertionError("not bootstrap " + name);
            System.out.println("CLASS\t" + name + "\t" + classHash(owner));
            for (Method method : owner.getDeclaredMethods()) {
                if (!method.isDefault() || method.isSynthetic()) continue;
                final List<String> abstractCalls = new ArrayList<>();
                Object proxy = Proxy.newProxyInstance(null, new Class<?>[] {owner}, (instance, invoked, parameters) -> {
                    abstractCalls.add(invoked.toGenericString());
                    throw new IllegalStateException("ABSTRACT_METHOD_REACHED");
                });
                MethodHandles.Lookup lookup = constructor.newInstance(owner, 15);
                java.lang.invoke.MethodHandle handle = lookup.unreflectSpecial(method, owner).bindTo(proxy);
                Object[] arguments = new Object[method.getParameterCount()];
                Class<?>[] parameters = method.getParameterTypes();
                for (int i = 0; i < arguments.length; i++) arguments[i] = argument(parameters[i]);
                String outcome; String detail; String state = "-";
                try {
                    Object result = handle.invokeWithArguments(arguments);
                    outcome = "RETURN"; detail = result == null ? "null" : String.valueOf(result);
                } catch (Throwable exception) {
                    if (!abstractCalls.isEmpty()) {
                        outcome = "ABSTRACT_DELEGATION"; detail = abstractCalls.get(0);
                    } else {
                        outcome = "THROW"; detail = exception.getClass().getName();
                        if (exception instanceof SQLException) {
                            String sqlState = ((SQLException) exception).getSQLState();
                            if (sqlState != null) state = sqlState;
                        }
                    }
                }
                System.out.println("DEFAULT\t" + method.toGenericString() + "\t" + outcome + "\t" + detail + "\t" + state + "\t" + abstractCalls.size());
                total++;
            }
        }
        if (total != 27) throw new AssertionError("expected 27 default methods, got " + total);
    }
}
