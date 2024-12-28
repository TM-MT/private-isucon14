# ヘルプを表示する用のスクリプト
define PRINT_HELP_PYSCRIPT
import sys, re
command_pattern = re.compile(r"^[a-zA-Z0-9\-_]+:")
lines = [line.rstrip() for line in sys.stdin if not line.startswith(".PHONY")]

for previous_line, current_line in zip(lines[:-1], lines[1:]):
	if command_pattern.match(current_line) and previous_line.startswith("# "):
		# コマンド行かつ、前の行が#から始まるコメント行であれば、コマンドとコメントを表示
		command = current_line.split(":")[0]
		comment = previous_line.lstrip("# ")
		print(f"{command:20s}\t{comment}")
	elif previous_line.startswith("## "):
		# ##から始まるコメント行があれば、緑字でセクションのタイトルを表示
		section_title = previous_line.lstrip("## ")
		print(f"\n\033[92m{section_title}\033[0m")
endef
export PRINT_HELP_PYSCRIPT

.PHONY: help
help:
	@python3 -c "$$PRINT_HELP_PYSCRIPT" < $(MAKEFILE_LIST)
	@# includeしたファイルも表示する場合は以下を使う
	@# @cat $(MAKEFILE_LIST) | python3 -u -c "$$PRINT_HELP_PYSCRIPT"

DATETIME_NOW := $(shell TZ=JST-9 date +%y%m%d_%H%M)
LOG_FILE_MYSQL := ./webapp/logs/mysql/mysql-slow.log
ERROR_FILE_MYSQL := ./webapp/logs/mysql/error.log

.PHONY: post-bench-mysql
post-bench-mysql:
	make analyze-mysql-log > ./webapp/logs/mysql/mysql-slow_$(DATETIME_NOW).log.analyze
	[ -s $(ERROR_FILE_MYSQL) ] && cp $(ERROR_FILE_MYSQL) logs/mysql/error_$(DATETIME_NOW).log || true


# mysqlのスロークエリログ解析
.PHONY: analyze-mysql-log
analyze-mysql-log:
	cat $(LOG_FILE_MYSQL) | \
		pt-query-digest \
			--limit 20 \
			--group-by fingerprint | less
