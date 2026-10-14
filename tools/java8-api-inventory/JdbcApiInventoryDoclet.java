import com.sun.javadoc.ClassDoc;
import com.sun.javadoc.ConstructorDoc;
import com.sun.javadoc.DocErrorReporter;
import com.sun.javadoc.FieldDoc;
import com.sun.javadoc.MethodDoc;
import com.sun.javadoc.Parameter;
import com.sun.javadoc.RootDoc;
import com.sun.javadoc.SourcePosition;
import com.sun.javadoc.Type;

import java.io.BufferedWriter;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Extracts public Java 8 declarations and their source inheritance edges. */
public final class JdbcApiInventoryDoclet {
    private static String outputPath;
    private static String sourceRoot;
    private static String sourceCommit;

    private JdbcApiInventoryDoclet() {
    }

    public static int optionLength(String option) {
        if ("-inventory-output".equals(option)
                || "-inventory-source-root".equals(option)
                || "-inventory-source-commit".equals(option)) {
            return 2;
        }
        return 0;
    }

    public static boolean validOptions(String[][] options, DocErrorReporter reporter) {
        for (String[] option : options) {
            if (option.length < 2) {
                continue;
            }
            if ("-inventory-output".equals(option[0])) {
                outputPath = option[1];
            } else if ("-inventory-source-root".equals(option[0])) {
                sourceRoot = option[1];
            } else if ("-inventory-source-commit".equals(option[0])) {
                sourceCommit = option[1];
            }
        }
        if (outputPath == null || sourceRoot == null || sourceCommit == null) {
            reporter.printError("inventory output, source root, and source commit are required");
            return false;
        }
        return true;
    }

    public static boolean start(RootDoc root) {
        try {
            Path rootPath = Paths.get(sourceRoot).toAbsolutePath().normalize();
            Map<String, ClassDoc> types = new HashMap<String, ClassDoc>();
            for (ClassDoc type : root.classes()) {
                collectTypes(type, types);
            }

            List<Record> records = new ArrayList<Record>();
            List<ClassDoc> orderedTypes = new ArrayList<ClassDoc>(types.values());
            Collections.sort(orderedTypes, (left, right) -> left.qualifiedName().compareTo(right.qualifiedName()));
            for (ClassDoc type : orderedTypes) {
                if (!isApiType(type)) {
                    continue;
                }
                addType(records, type, rootPath);
                addDeclaredMembers(records, type, rootPath);
                addInheritedMembers(records, type, types, rootPath);
            }

            Collections.sort(records, (left, right) -> left.sortKey().compareTo(right.sortKey()));
            Path target = Paths.get(outputPath).toAbsolutePath().normalize();
            Path parent = target.getParent();
            if (parent != null) {
                Files.createDirectories(parent);
            }
            try (BufferedWriter writer = Files.newBufferedWriter(target, StandardCharsets.UTF_8)) {
                writer.write("# schema=jdbc-java-method-inventory-v1\tsource_commit=" + escape(sourceCommit));
                writer.newLine();
                writer.write("kind\tidentity\towner\tdeclared_in\tname\tparameters\treturn_type\tmodifiers\tis_default\tconstant_value\tsource\tline");
                writer.newLine();
                for (Record record : records) {
                    writer.write(record.toTsv());
                    writer.newLine();
                }
            }
            return true;
        } catch (IOException | RuntimeException exception) {
            root.printError("unable to write Java method inventory: " + exception.getMessage());
            return false;
        }
    }

    private static void collectTypes(ClassDoc type, Map<String, ClassDoc> types) {
        if (type == null || types.containsKey(type.qualifiedName())) {
            return;
        }
        types.put(type.qualifiedName(), type);
        for (ClassDoc nested : type.innerClasses()) {
            collectTypes(nested, types);
        }
    }

    private static boolean isApiType(ClassDoc type) {
        return type.isPublic() || type.isProtected();
    }

    private static void addType(List<Record> records, ClassDoc type, Path rootPath) {
        String declarationKind = type.isAnnotationType()
                ? "annotation"
                : type.isEnum() ? "enum" : type.isInterface() ? "interface" : "class";
        List<String> parents = new ArrayList<String>();
        if (type.superclass() != null && !"java.lang.Object".equals(type.superclass().qualifiedName())) {
            parents.add(type.superclass().qualifiedName());
        }
        for (ClassDoc interfaceType : type.interfaces()) {
            parents.add(interfaceType.qualifiedName());
        }
        Collections.sort(parents);
        records.add(new Record(
                "type",
                type.qualifiedName() + "#TYPE",
                type.qualifiedName(),
                type.qualifiedName(),
                type.name(),
                "",
                declarationKind + (parents.isEmpty() ? "" : " extends/implements " + join(parents, ";")),
                type.modifiers(),
                "false",
                "",
                source(type.position(), rootPath),
                line(type.position())));
    }

    private static void addDeclaredMembers(List<Record> records, ClassDoc type, Path rootPath) {
        String owner = type.qualifiedName();
        for (ConstructorDoc constructor : type.constructors()) {
            if (!isApiMember(constructor)) {
                continue;
            }
            String parameters = parameters(constructor.parameters());
            records.add(new Record(
                    "constructor",
                    owner + "#<init>(" + parameters + ")",
                    owner,
                    owner,
                    constructor.name(),
                    parameters,
                    "",
                    constructor.modifiers(),
                    "false",
                    "",
                    source(constructor.position(), rootPath),
                    line(constructor.position())));
        }
        for (MethodDoc method : type.methods()) {
            if (!isApiMember(method) || !owner.equals(method.containingClass().qualifiedName())) {
                continue;
            }
            String parameters = parameters(method.parameters());
            records.add(new Record(
                    "method",
                    owner + "#" + method.name() + "(" + parameters + ")",
                    owner,
                    owner,
                    method.name(),
                    parameters,
                    method.returnType().toString(),
                    method.modifiers(),
                    Boolean.toString(method.isDefault()),
                    "",
                    source(method.position(), rootPath),
                    line(method.position())));
        }
        for (FieldDoc field : type.fields()) {
            if (!isApiMember(field) || !owner.equals(field.containingClass().qualifiedName())) {
                continue;
            }
            boolean enumConstant = field.isEnumConstant();
            boolean constant = enumConstant || (field.isStatic() && field.isFinal());
            if (!constant) {
                continue;
            }
            records.add(new Record(
                    enumConstant ? "enum_constant" : "constant",
                    owner + "#" + field.name(),
                    owner,
                    owner,
                    field.name(),
                    "",
                    field.type().toString(),
                    field.modifiers(),
                    "false",
                    nullToEmpty(field.constantValueExpression()),
                    source(field.position(), rootPath),
                    line(field.position())));
        }
    }

    private static void addInheritedMembers(
            List<Record> records,
            ClassDoc receiver,
            Map<String, ClassDoc> selectedTypes,
            Path rootPath) {
        Set<String> visitedTypes = new HashSet<String>();
        Set<String> emitted = new HashSet<String>();
        for (ClassDoc parent : parents(receiver)) {
            addInheritedFrom(records, receiver, parent, selectedTypes, visitedTypes, emitted, rootPath);
        }
    }

    private static void addInheritedFrom(
            List<Record> records,
            ClassDoc receiver,
            ClassDoc parent,
            Map<String, ClassDoc> selectedTypes,
            Set<String> visitedTypes,
            Set<String> emitted,
            Path rootPath) {
        if (parent == null || !selectedTypes.containsKey(parent.qualifiedName())
                || !visitedTypes.add(parent.qualifiedName())) {
            return;
        }
        String owner = receiver.qualifiedName();
        String declaringType = parent.qualifiedName();
        for (MethodDoc method : parent.methods()) {
            if (!isApiMember(method) || !declaringType.equals(method.containingClass().qualifiedName())) {
                continue;
            }
            String parameters = parameters(method.parameters());
            String identity = owner + "#" + method.name() + "(" + parameters + ")";
            String key = "method\0" + identity + "\0" + declaringType;
            if (emitted.add(key)) {
                records.add(new Record(
                        "inherited_method",
                        identity,
                        owner,
                        declaringType,
                        method.name(),
                        parameters,
                        method.returnType().toString(),
                        method.modifiers(),
                        Boolean.toString(method.isDefault()),
                        "",
                        source(method.position(), rootPath),
                        line(method.position())));
            }
        }
        for (FieldDoc field : parent.fields()) {
            if (!isApiMember(field) || !declaringType.equals(field.containingClass().qualifiedName())) {
                continue;
            }
            boolean enumConstant = field.isEnumConstant();
            if (!enumConstant && !(field.isStatic() && field.isFinal())) {
                continue;
            }
            String identity = owner + "#" + field.name();
            String key = "field\0" + identity + "\0" + declaringType;
            if (emitted.add(key)) {
                records.add(new Record(
                        "inherited_constant",
                        identity,
                        owner,
                        declaringType,
                        field.name(),
                        "",
                        field.type().toString(),
                        field.modifiers(),
                        "false",
                        nullToEmpty(field.constantValueExpression()),
                        source(field.position(), rootPath),
                        line(field.position())));
            }
        }
        for (ClassDoc ancestor : parents(parent)) {
            addInheritedFrom(records, receiver, ancestor, selectedTypes, visitedTypes, emitted, rootPath);
        }
    }

    private static List<ClassDoc> parents(ClassDoc type) {
        List<ClassDoc> parents = new ArrayList<ClassDoc>();
        if (type.superclass() != null && !"java.lang.Object".equals(type.superclass().qualifiedName())) {
            parents.add(type.superclass());
        }
        Collections.addAll(parents, type.interfaces());
        Collections.sort(parents, (left, right) -> left.qualifiedName().compareTo(right.qualifiedName()));
        return parents;
    }

    private static boolean isApiMember(com.sun.javadoc.ProgramElementDoc member) {
        return member.isPublic() || member.isProtected();
    }

    private static String parameters(Parameter[] parameters) {
        List<String> values = new ArrayList<String>();
        for (Parameter parameter : parameters) {
            Type type = parameter.type();
            values.add(type.toString());
        }
        return join(values, ",");
    }

    private static String source(SourcePosition position, Path rootPath) {
        if (position == null || position.file() == null) {
            return "";
        }
        Path file = position.file().toPath().toAbsolutePath().normalize();
        try {
            return rootPath.relativize(file).toString().replace(File.separatorChar, '/');
        } catch (IllegalArgumentException exception) {
            return file.getFileName().toString();
        }
    }

    private static String line(SourcePosition position) {
        return position == null ? "" : Integer.toString(position.line());
    }

    private static String join(List<String> values, String separator) {
        StringBuilder result = new StringBuilder();
        for (String value : values) {
            if (result.length() > 0) {
                result.append(separator);
            }
            result.append(value);
        }
        return result.toString();
    }

    private static String nullToEmpty(String value) {
        return value == null ? "" : value;
    }

    private static String escape(String value) {
        if (value == null) {
            return "";
        }
        return value.replace("\\", "\\\\")
                .replace("\t", "\\t")
                .replace("\r", "\\r")
                .replace("\n", "\\n");
    }

    private static final class Record {
        private final String kind;
        private final String identity;
        private final String owner;
        private final String declaredIn;
        private final String name;
        private final String parameters;
        private final String returnType;
        private final String modifiers;
        private final String isDefault;
        private final String constantValue;
        private final String source;
        private final String line;

        Record(
                String kind,
                String identity,
                String owner,
                String declaredIn,
                String name,
                String parameters,
                String returnType,
                String modifiers,
                String isDefault,
                String constantValue,
                String source,
                String line) {
            this.kind = kind;
            this.identity = identity;
            this.owner = owner;
            this.declaredIn = declaredIn;
            this.name = name;
            this.parameters = parameters;
            this.returnType = returnType;
            this.modifiers = modifiers;
            this.isDefault = isDefault;
            this.constantValue = constantValue;
            this.source = source;
            this.line = line;
        }

        String sortKey() {
            return kind + "\0" + identity + "\0" + declaredIn + "\0" + source + "\0" + line;
        }

        String toTsv() {
            return escape(kind) + "\t"
                    + escape(identity) + "\t"
                    + escape(owner) + "\t"
                    + escape(declaredIn) + "\t"
                    + escape(name) + "\t"
                    + escape(parameters) + "\t"
                    + escape(returnType) + "\t"
                    + escape(modifiers) + "\t"
                    + escape(isDefault) + "\t"
                    + escape(constantValue) + "\t"
                    + escape(source) + "\t"
                    + escape(line);
        }
    }
}
