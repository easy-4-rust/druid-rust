package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;

import javax.transaction.xa.Xid;
import java.util.Objects;
import java.util.Arrays;

/** Agent 协议中的不可变 Xid；保留 formatId、global transaction id 和 branch qualifier。 */
public final class AgentXid implements Xid {

    private final int formatId;
    private final byte[] globalTransactionId;
    private final byte[] branchQualifier;

    /** 创建防御性复制的 Xid。 */
    public AgentXid(int formatId, byte[] globalTransactionId, byte[] branchQualifier) {
        this.formatId = formatId;
        this.globalTransactionId = Objects.requireNonNull(globalTransactionId, "globalTransactionId").clone();
        this.branchQualifier = Objects.requireNonNull(branchQualifier, "branchQualifier").clone();
        if (this.globalTransactionId.length > Xid.MAXGTRIDSIZE
                || this.branchQualifier.length > Xid.MAXBQUALSIZE) {
            throw new IllegalArgumentException("Xid component exceeds XA maximum length");
        }
    }

    /** 从协议对象解码完整 Xid。 */
    public static AgentXid fromJson(JsonNode value) {
        Objects.requireNonNull(value, "xid");
        if (!value.isObject() || !value.path("formatId").canConvertToInt()) {
            throw new IllegalArgumentException("xid requires an integer formatId");
        }
        return new AgentXid(
                value.path("formatId").intValue(),
                bytes(value.get("globalTransactionId"), "globalTransactionId"),
                bytes(value.get("branchQualifier"), "branchQualifier"));
    }

    private static byte[] bytes(JsonNode value, String name) {
        if (Objects.isNull(value) || !value.isArray()) {
            throw new IllegalArgumentException("xid requires byte array " + name);
        }
        byte[] result = new byte[value.size()];
        for (int index = 0; index < value.size(); index++) {
            JsonNode element = value.get(index);
            if (!element.canConvertToInt() || element.intValue() < -128 || element.intValue() > 255) {
                throw new IllegalArgumentException("xid " + name + " contains a value outside byte range");
            }
            result[index] = (byte) element.intValue();
        }
        return result;
    }

    /** 编码完整 Xid，使用 0–255 表示无符号字节。 */
    public ObjectNode toJson() {
        ObjectNode result = com.fasterxml.jackson.databind.node.JsonNodeFactory.instance.objectNode();
        result.put("formatId", formatId);
        putBytes(result.putArray("globalTransactionId"), globalTransactionId);
        putBytes(result.putArray("branchQualifier"), branchQualifier);
        return result;
    }

    private void putBytes(ArrayNode target, byte[] bytes) {
        for (byte value : bytes) {
            target.add(Byte.toUnsignedInt(value));
        }
    }

    @Override
    public int getFormatId() {
        return formatId;
    }

    @Override
    public byte[] getGlobalTransactionId() {
        return globalTransactionId.clone();
    }

    @Override
    public byte[] getBranchQualifier() {
        return branchQualifier.clone();
    }

    @Override
    public boolean equals(Object other) {
        if (this == other) {
            return true;
        }
        if (!(other instanceof Xid xid)) {
            return false;
        }
        return formatId == xid.getFormatId()
                && Arrays.equals(globalTransactionId, xid.getGlobalTransactionId())
                && Arrays.equals(branchQualifier, xid.getBranchQualifier());
    }

    @Override
    public int hashCode() {
        int result = Integer.hashCode(formatId);
        result = 31 * result + Arrays.hashCode(globalTransactionId);
        result = 31 * result + Arrays.hashCode(branchQualifier);
        return result;
    }
}
