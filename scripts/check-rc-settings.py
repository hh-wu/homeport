import sqlite3

DB = r"D:\Programs\SageportRC\data\sageport.db"
con = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
rows = con.execute(
    "SELECT key, value FROM settings WHERE key LIKE 'remote.%' ORDER BY key"
).fetchall()
print(f"本地数据库中的 remote.* 配置（{len(rows)} 项）：")
for key, value in rows:
    shown = value if len(value) <= 46 else value[:43] + "..."
    print(f"  {key:30} = {shown}")
con.close()
