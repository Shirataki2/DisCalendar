import asyncio
import discord
from discord.ext import commands
from discal.bot import Bot


class Report(commands.Cog):
    LOG_CHANNEL = 783690714978451486

    def __init__(self, bot):
        self.bot: Bot = bot

    def _same_user_check(self, ctx):
        return lambda message: message.author == ctx.author

    async def send_report(self, ctx, content):
        logch = self.bot.get_channel(self.LOG_CHANNEL)
        embed = discord.Embed(color=0xff0000)
        embed.title = 'DisCalendar - Report'
        embed.description = f'```\n{content}\n```'
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

    @commands.command()
    async def report(self, ctx: commands.Context):
        embed = discord.Embed(color=0x0000ff)
        embed.title = 'DisCalendar - Report'
        embed.description = (
            'DisCalendarのご利用ありがとうございます\n\n'
            'このコマンドではDisCalendarに対する要望やバグを'
            '開発者に報告することが可能です\n\n'
            '**次にあなたが送信したメッセージの内容が開発者に送信されます**\n\n'
            '(キャンセルの際は`\\c`とご入力ください)\n\n'
            '🌟10分以内にご送信ください．🌟\n10分経過すると自動的にキャンセルされます'
        )
        msg = await ctx.send(embed=embed)
        try:
            user_msg = await self.bot.wait_for('message', check=self._same_user_check(ctx), timeout=600)
        except asyncio.TimeoutError:
            embed.description = '```\nキャンセルしました\n```'
            return await msg.edit(embed=embed)
        if user_msg.content == r'\c':
            embed.description = '```\nキャンセルしました\n```'
            return await msg.edit(embed=embed)
        await self.send_report(ctx, user_msg.content)
        embed.description = '```\n報告が完了しました\n```'
        return await msg.edit(embed=embed)


def setup(bot):
    bot.add_cog(Report(bot))
