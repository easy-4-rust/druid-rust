package fixture.api;

public interface ConnectionEventListener {
    void connectionClosed();
    void connectionErrorOccurred(String sqlState);
    default String ownerName() { return "owner"; }
    String classify();
    String classify(String input);
}
