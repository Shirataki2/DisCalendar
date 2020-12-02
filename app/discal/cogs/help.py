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


class HelpCommand(commands.Cog):
    def __init__(self, bot):
        self.bot: Bot = bot

    def _create_help(self):
        embed = discord.Embed(color=0x0000ff)
        embed.title = 'DisCalendar - Help'
        embed.description = (
            'DisCalendarはDiscord用の__予定管理Bot__です\n\n'
            'ほとんどの操作は**Web上で行えることが特徴です！**\n\n'
            '[**こちら**](https://discalendar.app)からログインして'
            'サーバーのカレンダーを閲覧することができます\n\n'
            '__**🌟初期化🌟**__\n'
            'この操作を行わなくても予定の追加はできますが'
            '追加した予定の開始時間になった際にチャンネルに'
            '投稿するようにするには初期化処理が必要です！\n\n'
            'このBotからメッセージを受信したいチャンネルで\n'
            '```\ncal init\n```\nと入力してください\n\n'
            '受信チャンネルを変更したい際には再度別のチャンネルで'
            'このコマンドを実行して下さい\n\n'
            '__**🌟サポートサーバー🌟**__\n'
            '機能要望やバグなどがあった場合には'
            '[サポートサーバー](https://discord.gg/YF4E8mDr9Z)へ参加し，ご連絡をお願いします！\n\n'
            'もしくは，`cal report`コマンドでも報告を行うことができます！\n\n'
            '__**🌟他のサーバーにも導入する場合🌟**__\n'
            f'[こちら]({os.environ["INVITATION_URL"]})より導入をお願いします！'
        )
        embed.set_footer(text=f'v{discal.__version__}')
        embed.timestamp = datetime.utcnow()
        embed.set_thumbnail(url=self.bot.user.avatar_url)
        return embed

    @commands.command()
    async def help(self, ctx: commands.Context):
        await ctx.send(embed=self._create_help())
    
    @commands.Cog.listener()
    async def on_message(self, message: discord.Message):
        if message.author.bot:
            return
        if self.bot.user in message.mentions:
            await message.channel.send(embed=self._create_help())


def setup(bot):
    bot.add_cog(HelpCommand(bot))