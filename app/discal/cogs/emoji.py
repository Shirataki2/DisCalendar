import asyncio
import json
import discord
import os
from discord.ext import commands
from datetime import datetime
import discal
from discal.bot import Bot
from discal.logger import get_module_logger


logger = get_module_logger(__name__)


class Emoji(commands.Cog):
    CHECK = 782497837975076884
    CROSS = 782497837706641419


    def __init__(self, bot):
        self.bot: Bot = bot

    async def cog_check(self, ctx):
        return await self.bot.is_owner(ctx.author)

    @commands.Cog.listener()
    async def on_ready(self):
        self.bot.e_check = self.bot.get_emoji(self.CHECK)
        self.bot.e_cross = self.bot.get_emoji(self.CROSS)


def setup(bot):
    bot.add_cog(Emoji(bot))
