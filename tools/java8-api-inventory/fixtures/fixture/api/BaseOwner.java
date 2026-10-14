package fixture.api;

public abstract class BaseOwner {
    public BaseOwner() {}
    protected BaseOwner(String name) {}
    public void closeOwner() {}
    private void internalOnly() {}
}
