import asyncio
import json
import discord
import jpholiday
from discord.ext import commands, tasks
from discal.bot import Bot
from datetime import datetime, timedelta
from discal.logger import get_module_logger


logger = get_module_logger(__name__)

def get_datetype(date):
    if date.weekday() == 6 or jpholiday.is_holiday(Date):
        return "_r"
    elif date.weekday() == 5:
        return "_b"
    else:
        return ""

class Updater(commands.Cog):
    def __init__(self, bot):
        self.bot: Bot = bot
        self.postloop.start()

    def cog_unload(self):
        self.postloop.cancel()

    @tasks.loop(minutes=1)
    async def postloop(self):
        now = datetime.now() + timedelta(hours=9)
        if now.hour == 0 and now.minute == 0:
            guild = self.bot.get_guild(782168943967469569)
            logger.info('Change Logo')
            suffix = get_datetype(now)
            day = now.day
            fn = f"dates/{day}{suffix}.png"
            with open(fn, 'rb') as fp:
                b = fp.read()
                await self.bot.user.edit(avatar=b)
                await guild.edit(icon=b)

    @postloop.before_loop
    async def wait_ready(self):
        logger.info('waiting...')
        await self.bot.wait_until_ready()


def setup(bot):
    bot.add_cog(Updater(bot))
