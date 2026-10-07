# EDB Local AI Skill

`edb local` 用于调试**未在区块浏览器验证**的本地 Foundry 项目合约。

## AI 的完整工作流程

### 1. 启动 Anvil

```bash
# 检查是否已运行
curl -s -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  http://localhost:8545 > /dev/null 2>&1 && echo "Anvil already running" || (anvil --port 8545 > /dev/null 2>&1 & sleep 2 && echo "Anvil started")
```

### 2. 进入 Foundry 项目

```bash
cd /path/to/foundry/project
```

### 3. 生成模板文件

```bash
edb local . --init
```

生成两个文件：
- `edb.local.json` — 合约配置（需要 AI 填入实际值）
- `edb-setup.sh` — 部署脚本（需要 AI 自定义）

### 4. 部署合约

```bash
# 部署合约并获取地址和交易哈希
forge create src/YourContract.sol:YourContract \
  --rpc-url http://localhost:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --broadcast

# 从输出中提取：
# - Deployed to: <CONTRACT_ADDRESS>
# - Transaction hash: <CREATION_TX_HASH>
```

### 5. 更新 edb.local.json

AI 必须编辑 `edb.local.json`，填入：
- `address`: 步骤 4 的合约地址
- `name`: 合约名称（必须与 Solidity 合约名匹配）
- `source`: 源码路径（相对于项目根目录）
- `creation_tx`: 步骤 4 的部署交易哈希
- `test_transaction.to`: 合约地址
- `test_transaction.function`: 要调试的函数签名
- `test_transaction.args`: 函数参数

示例：
```json
{
  "project_root": ".",
  "solc_version": "0.8.26",
  "contracts": [
    {
      "address": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
      "name": "Counter",
      "source": "src/Counter.sol",
      "creation_tx": "0x6969877ecd7ccf54b774ca32cf9ca5127976d643b0bab4258d10d49923d0d887",
      "constructor_args": "0x"
    }
  ],
  "test_transaction": {
    "to": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
    "function": "increment()",
    "args": []
  }
}
```

### 6. 执行测试交易

```bash
cast send <CONTRACT_ADDRESS> "functionName(args)" \
  --rpc-url http://localhost:8545 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --json | jq -r '.transactionHash'
```

记录输出的交易哈希。

### 7. 调试交易

```bash
edb local . --no-anvil --tx-hash <TX_HASH>
```

## 关键要点

1. **Anvil 必须先启动** — 所有部署和交易都在本地 anvil 上执行
2. **必须使用 --broadcast** — `forge create` 需要 `--broadcast` 才能真正部署
3. **必须更新 edb.local.json** — 生成的模板是占位符，AI 必须填入实际值
4. **合约名称必须匹配** — `name` 字段必须与 Solidity 合约名完全一致
5. **solc_version 必须匹配** — 确保 `foundry.toml` 和 `edb.local.json` 的 solc 版本一致
