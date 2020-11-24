from discal.bot import Bot
import os


Bot(command_prefix="cal ").run(os.environ["BOT_TOKEN"])
