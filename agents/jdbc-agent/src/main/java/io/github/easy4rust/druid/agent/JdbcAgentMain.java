package io.github.easy4rust.druid.agent;

import lombok.extern.slf4j.Slf4j;

/** Druid JDBC Agent 独立进程入口。 */
@Slf4j
public final class JdbcAgentMain {

    private JdbcAgentMain() {
    }

    /**
     * 在标准输入输出上运行 DAP1 帧循环；日志只写标准错误。
     *
     * @param args 保留参数
     */
    public static void main(String[] args) {
        try (AgentServer server = new AgentServer(System.in, System.out)) {
            server.run();
        } catch (Exception exception) {
            log.error("Druid JDBC Agent terminated", exception);
            System.exit(1);
        }
    }
}
