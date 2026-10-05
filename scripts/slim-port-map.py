"""端口映射只保留「对面机器上的端口」，本机 visitor 的端口由访客配置自动提供。"""

import sqlite3

DB = r"D:\Programs\Homeport\data\homeport.db"
VALUE = "home-rdp = 13389\nhome-ssh = 10022"

con = sqlite3.connect(DB)
con.execute("UPDATE settings SET value = ? WHERE key = 'remote.portMap'", (VALUE,))
con.commit()
row = con.execute("SELECT value FROM settings WHERE key = 'remote.portMap'").fetchone()
print("remote.portMap =")
for line in (row[0] if row else "").splitlines():
    print(f"  {line}")
con.close()
