import discord
from discord.ext import commands
from discal.bot import Bot
from discal.logger import get_module_logger


logger = get_module_logger(__name__)


class Register(commands.Cog):
    def __init__(self, bot):
        self.bot: Bot = bot

    @commands.command()
    async def init(self, ctx: commands.Context):
        setting = await self.bot.pool.fetchrow(
            'SELECT * FROM event_settings ' +
            'WHERE guild_id = $1;',
            str(ctx.guild.id)
        )
        if setting is None:
            await self.bot.pool.execute(
                'INSERT INTO event_settings' +
                '(guild_id, channel_id) ' +
                'VALUES ($1, $2);',
                str(ctx.guild.id), str(ctx.channel.id)
            )
            await ctx.send(f'通知の投稿先を{ctx.channel.mention}に設定しました')
        else:
            await self.bot.pool.execute(
                'UPDATE event_settings ' +
                'SET channel_id = $2 ' +
                'WHERE guild_id = $1;',
                str(ctx.guild.id), str(ctx.channel.id)
            )
            await ctx.send(f'通知の投稿先を{ctx.channel.mention}に変更しました')

    @commands.Cog.listener()
    async def on_guild_join(self, guild: discord.Guild):
        try:
            await self.bot.pool.execute(
                'INSERT INTO guilds(guild_id, name, avatar_url) VALUES ($1, $2, $3)',
                str(guild.id), guild.name, str(guild.icon_url_as(static_format='jpg', size=256))
            )
            logger.info(f'New Guild: {guild.name} ({guild.id}) joined!')
        except Exception as e:
            logger.error(f'Failed to add record: {guild.name} ({guild.id})')
            raise e

    
    @commands.Cog.listener()
    async def on_guild_remove(self, guild: discord.Guild):
        try:
            await self.bot.pool.execute(
                'DELETE FROM guilds WHERE guild_id = $1',
                str(guild.id)
            )
            logger.info(f'Guild: {guild.name} ({guild.id}) kicked this bot or has been deleted!')
        except:
            logger.error(f'Failed to remove record: {guild.name} ({guild.id})')


def setup(bot):
    bot.add_cog(Register(bot))
