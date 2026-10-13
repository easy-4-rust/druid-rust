package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;

import javax.sql.XADataSource;
import java.beans.Introspector;
import java.beans.PropertyDescriptor;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.sql.SQLException;
import java.util.Iterator;
import java.util.Map;
import java.util.Objects;

/** 按显式类名和 JavaBean 属性创建 XADataSource；不从 JDBC URL 推断 XA 能力。 */
public final class XaDataSourceFactory {

    private XaDataSourceFactory() {
    }

    /** 实例化并配置调用方明确指定的 XADataSource。 */
    public static XADataSource create(String className, JsonNode properties) throws SQLException {
        if (Objects.isNull(className) || className.isBlank()) {
            throw new SQLException("XADataSource className is required");
        }
        try {
            Class<?> type = Class.forName(className);
            if (!XADataSource.class.isAssignableFrom(type)) {
                throw new SQLException("Configured class does not implement javax.sql.XADataSource");
            }
            XADataSource dataSource = (XADataSource) type.getDeclaredConstructor().newInstance();
            configure(dataSource, properties);
            return dataSource;
        } catch (SQLException exception) {
            throw exception;
        } catch (ReflectiveOperationException | RuntimeException exception) {
            throw new SQLException("Unable to create configured XADataSource", exception);
        }
    }

    private static void configure(XADataSource dataSource, JsonNode properties) throws SQLException {
        if (Objects.isNull(properties) || !properties.isObject()) {
            throw new SQLException("XADataSource properties must be an object");
        }
        try {
            Map<String, PropertyDescriptor> descriptors = new java.util.HashMap<>();
            for (PropertyDescriptor descriptor : Introspector.getBeanInfo(dataSource.getClass())
                    .getPropertyDescriptors()) {
                descriptors.put(descriptor.getName(), descriptor);
            }
            Iterator<Map.Entry<String, JsonNode>> fields = properties.fields();
            while (fields.hasNext()) {
                Map.Entry<String, JsonNode> field = fields.next();
                PropertyDescriptor descriptor = descriptors.get(field.getKey());
                Method setter = Objects.isNull(descriptor) ? null : descriptor.getWriteMethod();
                if (Objects.isNull(setter)) {
                    throw new SQLException("XADataSource property has no setter: " + field.getKey());
                }
                setter.invoke(dataSource, convert(field.getValue(), setter.getParameterTypes()[0], field.getKey()));
            }
        } catch (SQLException exception) {
            throw exception;
        } catch (java.beans.IntrospectionException | ReflectiveOperationException | RuntimeException exception) {
            Throwable cause = exception instanceof InvocationTargetException invocation
                    ? invocation.getTargetException()
                    : exception;
            throw new SQLException("Unable to configure XADataSource", cause);
        }
    }

    private static Object convert(JsonNode value, Class<?> target, String name) throws SQLException {
        if (target == String.class && value.isTextual()) {
            return value.textValue();
        }
        if ((target == int.class || target == Integer.class) && value.canConvertToInt()) {
            return value.intValue();
        }
        if ((target == long.class || target == Long.class) && value.canConvertToLong()) {
            return value.longValue();
        }
        if ((target == boolean.class || target == Boolean.class) && value.isBoolean()) {
            return value.booleanValue();
        }
        if (target == String[].class && value.isArray()) {
            String[] result = new String[value.size()];
            for (int index = 0; index < value.size(); index++) {
                if (!value.get(index).isTextual()) {
                    throw new SQLException("XADataSource property array must contain strings: " + name);
                }
                result[index] = value.get(index).textValue();
            }
            return result;
        }
        if (target == int[].class && value.isArray()) {
            int[] result = new int[value.size()];
            for (int index = 0; index < value.size(); index++) {
                if (!value.get(index).canConvertToInt()) {
                    throw new SQLException("XADataSource property array must contain integers: " + name);
                }
                result[index] = value.get(index).intValue();
            }
            return result;
        }
        throw new SQLException("XADataSource property has an unsupported value type: " + name);
    }
}
