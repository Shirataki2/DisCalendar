from discord.ext import commands

class Miscs(commands.Cog):
    def __init__(self, bot):
        self.bot = bot


def setup(bot):
    bot.add_cog(Miscs(bot))
