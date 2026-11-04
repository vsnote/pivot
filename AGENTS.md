Avoid unnecessary secondary labels and descriptions. Let the layout do the explaining.

# 核心架构与业务准则

## 路由与模型机制（铁律）
- Pivot 不设全局「默认模型」或「默认服务商」，各 Agent/客户端在列表中独立按服务商选择启动模型。
- 服务商只负责声明可用模型池与连接凭据，不设默认模型。
- 网关路由严格按照请求的模型精确/前缀/归一化分发到声明了该模型的服务商；若无任何服务商支持该模型，直接报错返回，绝不进行静默跨模型降级或篡改。
