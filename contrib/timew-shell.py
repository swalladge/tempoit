#!/usr/bin/env python
# Copyright © 2020 Samuel Walladge

# Smart interactive wrapper around timewarrior to save keystrokes.

import readline
import shlex
import subprocess


class Style:
    GREEN = "\033[32m"
    RED = "\033[31m"
    BOLD = "\033[1m"
    RESET = "\033[0m"


def main():
    return_code = 0
    while True:
        try:
            colour = Style.GREEN if return_code == 0 else Style.RED
            line = input(f"{Style.BOLD}{colour}timew ❯{Style.RESET} ")
        except EOFError:
            return

        try:
            argsv = shlex.split(line.strip())
        except Exception as e:
            print(repr(e))
            continue

        if argsv == []:
            cmd = ["timew", ":id", ":ann", "summary"]
        elif len(argsv) > 0 and argsv[0] == "te":
            cmd = ["tempoit"] + argsv[1:]
        elif len(argsv) > 0 and argsv[0] == "ocs":
            cmd = ["timew", ":id", ":ann", "start", "oc", "log"] + argsv[1:]
        elif len(argsv) > 0 and argsv[0] == "t":
            cmd = ["timew", ":id", ":ann", "tag"] + argsv[1:]
        else:
            cmd = ["timew", ":id", ":ann"] + argsv

        proc = subprocess.run(cmd)
        return_code = proc.returncode


if __name__ == "__main__":
    main()
