import java.io.InputStream;
import java.security.MessageDigest;
import java.lang.reflect.Method;
import java.lang.reflect.Field;
import java.lang.reflect.Constructor;
import java.lang.reflect.Modifier;
import java.util.Arrays;

public final class Jdbc42SignatureOracle {
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
    public static void main(String[] names) throws Exception {
        for (String name : names) {
            Class<?> type = Class.forName(name, false, null);
            if (type.getClassLoader() != null) throw new AssertionError("not bootstrap class " + name);
            String kind = type.isEnum() ? "enum" : type.isInterface() ? "interface" : "class";
            System.out.println("CLASS\t" + name + "\t" + kind + "\t" + classHash(type));
            for (Method method : type.getDeclaredMethods()) {
                if (Modifier.isPublic(method.getModifiers()) && !method.isSynthetic())
                    System.out.println("METHOD\t" + name + "\t" + method.isDefault() + "\t" + method.toGenericString());
            }
            for (Field field : type.getDeclaredFields()) {
                if (Modifier.isPublic(field.getModifiers()) && !field.isSynthetic())
                    System.out.println("FIELD\t" + name + "\t" + field.toGenericString());
            }
            for (Constructor<?> constructor : type.getDeclaredConstructors()) {
                if (Modifier.isPublic(constructor.getModifiers()) && !constructor.isSynthetic())
                    System.out.println("CTOR\t" + name + "\t" + constructor.toGenericString());
            }
        }
    }
}
