from discord.ext import commands
from discal.bot import Bot
from discal.logger import get_module_logger


logger = get_module_logger(__name__)


class Emoji(commands.Cog):
    CHECK = 782497837975076884
    CROSS = 782497837706641419
    DUMMY = 784875245471399957
    SQ_CHECK = 785019333561942026
    SQ_CROSS = 785019333629313064

    def __init__(self, bot):
        self.bot: Bot = bot

    async def cog_check(self, ctx):
        return await self.bot.is_owner(ctx.author)

    @commands.Cog.listener()
    async def on_ready(self):
        self.bot.e_check = self.bot.get_emoji(self.CHECK)
        self.bot.e_cross = self.bot.get_emoji(self.CROSS)
        self.bot.e_dummy = self.bot.get_emoji(self.DUMMY)
        self.bot.e_sq_check = self.bot.get_emoji(self.SQ_CHECK)
        self.bot.e_sq_cross = self.bot.get_emoji(self.SQ_CROSS)


def setup(bot):
    bot.add_cog(Emoji(bot))
