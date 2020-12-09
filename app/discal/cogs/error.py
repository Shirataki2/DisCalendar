import discord
import traceback
from discord.ext import commands
from datetime import datetime
from discal.bot import Bot
from discal.logger import get_module_logger


logger = get_module_logger(__name__)


async def send_error(ctx, title, message=None, **kwargs):
    embed = discord.Embed(color=0xff0000, **kwargs)
    embed.title = title
    if message:
        embed.description = message
    embed.timestamp = datetime.utcnow()
    embed.set_author(name='DisCalendar - Error')
    await ctx.send(embed=embed)


class Error(commands.Cog):
    LOGCH = 783737174582624286

    def __init__(self, bot):
        self.bot: Bot = bot
        self.bot.send_error = send_error

    async def cog_check(self, ctx):
        return await self.bot.is_owner(ctx.author)

    async def send_error_log(self, ctx, **kwargs):
        embed = discord.Embed(color=0xff0000, **kwargs)
        logch = self.bot.get_channel(self.LOGCH)
        logger.error(f'[guild  : {ctx.guild.name} / {ctx.guild.id}]')
        logger.error(f'[user   : {ctx.author.name} / {ctx.author.id}]')
        logger.error(f'[channel: {ctx.channel.name} / {ctx.channel.id}]')
        logger.error(f'[message: {ctx.message.id}]')
        if ctx.guild:
            embed.add_field(
                name='Guild',
                value=f'{ctx.guild.name} / {ctx.guild.id}'
            )
        embed.add_field(
            name='User',
            value=f'{ctx.author.name} / {ctx.author.id}'
        )
        embed.add_field(
            name='Message',
            value=f'{ctx.message.id} at {ctx.channel.name} / {ctx.channel.id}'
        )
        await logch.send(embed=embed)

    @commands.Cog.listener()
    async def on_command_error(self, ctx, error):
        if isinstance(error, commands.CommandNotFound):
            return
        logger.error(f'{error.__class__.__name__}')
        await self.send_error_log(
            ctx,
            title=f'{error.__class__.__name__}',
            description=f'```\n{error}\n```',
        )
        try:
            raise error
        except Exception:
            exc = traceback.format_exc().split('\n')
            for line in exc:
                logger.error(line)


def setup(bot):
    bot.add_cog(Error(bot))
