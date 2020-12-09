import discord
import os
from discord.ext import commands
from datetime import datetime
import discal
from discal.bot import Bot
from discal.logger import get_module_logger


logger = get_module_logger(__name__)


class HelpCommand(commands.Cog):
    def __init__(self, bot):
        self.bot: Bot = bot

    def _create_help(self):
        embed = discord.Embed(color=0x0000ff)
        embed.title = 'DisCalendar - Help'
        s = self.bot.e_dummy
        embed.description = (
            f'DisCalendarはDiscord用の__予定管理Bot__です\n\n'
            f'ほとんどの操作は**Web上で行えることが特徴です！**\n\n'
            f'[**こちら**](https://discalendar.app)からログインして'
            f'サーバーのカレンダーを閲覧することができます\n\n'
            f'__**🌟初期化🌟**__\n'
            f'{s}この操作を行わなくても予定の追加はできますが'
            f'追加した予定の開始時間になった際にチャンネルに'
            f'投稿するようにするには初期化処理が必要です！\n\n'
            f'{s}このBotからメッセージを受信したいチャンネルで\n'
            f'{s}```\ncal init\n```\n{s}と入力してください\n\n'
            f'{s}受信チャンネルを変更したい際には再度別のチャンネルで'
            f'このコマンドを実行して下さい\n\n'
            f'__**🌟コマンド機能🌟**__\n'
            f'{s}Discord上でも予定の表示が行えます！\n'
            f'{s}詳しい機能は`cal cli`で見ることができます！\n\n'
            f'__**🌟サポートサーバー🌟**__\n'
            f'{s}機能要望やバグなどがあった場合には'
            f'[サポートサーバー](https://discord.gg/MyaZRuze23)へ参加し，ご連絡をお願いします！\n\n'
            f'{s}もしくは，`cal report`コマンドでも報告を行うことができます！\n\n'
            f'__**🌟他のサーバーにも導入する場合🌟**__\n'
            f'{s}[こちら]({os.environ["INVITATION_URL"]})より導入をお願いします！'
        )
        embed.set_footer(text=f'v{discal.__version__}')
        embed.timestamp = datetime.utcnow()
        embed.set_thumbnail(url=self.bot.user.avatar_url)
        return embed

    @commands.group()
    async def help(self, ctx: commands.Context):
        if ctx.invoked_subcommand is None:
            await ctx.send(embed=self._create_help())

    @help.command()
    async def cli(self, ctx: commands.Context):
        cli = self.bot.get_cog('CLI')
        embed = cli.create_cli_help()
        await ctx.send(embed=embed)

    @commands.Cog.listener()
    async def on_message(self, message: discord.Message):
        if message.author.bot:
            return
        if self.bot.user in message.mentions:
            print(message.content)
            await message.channel.send(embed=self._create_help())


def setup(bot):
    bot.add_cog(HelpCommand(bot))
