import discord
from discord.ext import commands
from discal.bot import Bot
from discal.logger import get_module_logger


logger = get_module_logger(__name__)


class Miscs(commands.Cog):
    def __init__(self, bot):
        self.bot: Bot = bot

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
    bot.add_cog(Miscs(bot))
