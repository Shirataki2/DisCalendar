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


class Error(commands.Cog):
    LOGCH = 783740453173985280


    def __init__(self, bot):
        self.bot: Bot = bot

    async def cog_check(self, ctx):
        return await self.bot.is_owner(ctx.author)

    async def send_log(self, embed):
        logch = self.bot.get_channel(self.LOGCH)
        await logch.send(embed=embed)

    @commands.Cog.listener()
    async def on_guild_join(self, guild: discord.Guild):
        embed = discord.Embed(color=0x0000ff)
        embed.title = 'DisCalendar - New Guild'
        embed.description = f'{guild.name} ({guild.id})'
        embed.timestamp = datetime.utcnow()
        await self.send_log(embed)

    @commands.Cog.listener()
    async def on_guild_remove(self, guild: discord.Guild):
        embed = discord.Embed(color=0x00ff00)
        embed.title = 'DisCalendar - Leave Guild'
        embed.description = f'{guild.name} ({guild.id})'
        embed.timestamp = datetime.utcnow()
        await self.send_log(embed)


def setup(bot):
    bot.add_cog(Error(bot))
