"""把面板的访客配置文件指向实际部署路径。"""

import sqlite3

DB = r"D:\Programs\Homeport\data\homeport.db"
VALUE = r"D:\Programs\frp\frpc.toml"

con = sqlite3.connect(DB)
con.execute(
    "UPDATE settings SET value = ? WHERE key = 'remote.visitorConfigPath'", (VALUE,)
)
con.commit()
row = con.execute(
    "SELECT value FROM settings WHERE key = 'remote.visitorConfigPath'"
).fetchone()
print(f"remote.visitorConfigPath = {row[0] if row else '(缺失)'}")
con.close()
