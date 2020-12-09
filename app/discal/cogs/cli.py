import discord
import discal
import json
from discord.ext import commands
from discal.bot import Bot
from discal.logger import get_module_logger
from discal.cogs.utils.paginator import EmbedPaginator
from datetime import datetime, timedelta
logger = get_module_logger(__name__)

JST = timedelta(hours=9)


def valid_date(msg):
    s = msg.content
    try:
        y, m, d = map(int, s.split('-'))
        if m >= 13 or y <= 2019 or d >= 32:
            return False
        return datetime(y, m, d)
    except Exception:
        return False


def valid_datetime(msg):
    s = msg.content
    try:
        y, m, d, H, M = map(int, s.split('-'))
        if m >= 13 or y <= 2019 or d >= 32 or H >= 25 or M >= 60:
            return False
        return datetime(y, m, d, H, M)
    except Exception:
        return False


def preview_date(date, all_day=False):
    if all_day:
        return date.strftime('%Y/%m/%d')
    else:
        return date.strftime('%Y/%m/%d %H:%M')


class CLI(commands.Cog):
    def __init__(self, bot):
        self.bot: Bot = bot

    def create_cli_help(self):
        embed = discord.Embed(color=0x0000ff)
        embed.title = 'DisCalendar - CLI'
        embed.set_thumbnail(url=self.bot.user.avatar_url)
        embed.description = (
            'わざわざWebブラウザを起動せずともDiscord内で'
            '予定を見ることもできます'
        )
        s = self.bot.e_dummy
        embed.add_field(
            name='・cal cli list',
            value=(
                f'{s}予定の一覧を表示することができます\n\n'
                f'{s}`cal cli list past`{s}で過去の予定を\n'
                f'{s}`cal cli list`{s}{s}{s}または\n'
                f'{s}`cal cli list future` で未来の予定を\n'
                f'{s}`cal cli list all`{s} で全ての予定を確認することもできます\n'
            )
        )
        embed.set_footer(text=f'v{discal.__version__}')
        embed.timestamp = datetime.utcnow()
        return embed

    def show_date(self, event):
        start = event['start_at'].strftime('%Y/%m/%d %H:%M')
        end = event['end_at'].strftime('%Y/%m/%d %H:%M')
        end_t = event['end_at'].strftime('%H:%M')
        start_d = event['start_at'].strftime('%Y/%m/%d')
        end_d = event['end_at'].strftime('%Y/%m/%d')
        is_all_day = event['is_all_day']
        if is_all_day and start_d == end_d:
            return start_d
        if is_all_day and start_d != end_d:
            return f'{start_d} - {end_d}'
        if not is_all_day and start_d == end_d:
            return f'{start} - {end_t}'
        if not is_all_day and start_d != end_d:
            return f'{start} - {end}'

    def show_notification(self, event):
        notifications = []
        for notification in event['notifications']:
            r = json.loads(notification)
            notifications.append(f'{r["num"]}{r["type"]}')
        return ', '.join(notifications)

    @commands.group(invoke_without_command=False)
    async def cli(self, ctx: commands.Context):
        if ctx.invoked_subcommand is None:
            embed = self.create_cli_help()
            await ctx.send(embed=embed)

    @cli.group(name='list', invoke_without_command=False)
    async def _list(self, ctx: commands.Context):
        if ctx.invoked_subcommand is None:
            await ctx.invoke(self.bot.get_command('cli list future'))

    @_list.command(name='future', aliases=['ftr'])
    async def list_future(self, ctx: commands.Context):
        results = await self.bot.pool.fetch(
            (
                'SELECT * FROM events '
                'WHERE guild_id = $1 AND '
                'start_at >= $2 '
                'ORDER BY start_at'
            ),
            str(ctx.guild.id),
            datetime.utcnow() + JST
        )
        paginator = EmbedPaginator(color=0x0000ff)
        paginator.title = 'DisCalendar - CLI'
        paginator.description = '将来の予定を表示しています'
        for event in results:
            paginator.add_line(
                event['name'],
                (
                    f'`ＩＤ`: {event["id"]}\n'
                    f'`日時`: {self.show_date(event)}\n'
                    f'`通知`: {self.show_notification(event)}\n'
                )
            )
        await paginator.paginate(ctx, 5)

    @_list.command(name='past', aliases=['pst'])
    async def list_past(self, ctx: commands.Context):
        results = await self.bot.pool.fetch(
            (
                'SELECT * FROM events '
                'WHERE guild_id = $1 AND '
                'start_at <= $2 '
                'ORDER BY start_at DESC;'
            ),
            str(ctx.guild.id),
            datetime.utcnow() + JST
        )
        paginator = EmbedPaginator(color=0x0000ff)
        paginator.title = 'DisCalendar - CLI'
        paginator.description = '過去の予定を表示しています'
        for event in results:
            paginator.add_line(
                event['name'],
                (
                    f'`ＩＤ`: {event["id"]}\n'
                    f'`日時`: {self.show_date(event)}\n'
                    f'`通知`: {self.show_notification(event)}\n'
                )
            )
        await paginator.paginate(ctx, 5)

    @_list.command(name='all')
    async def list_all(self, ctx: commands.Context):
        results = await self.bot.pool.fetch(
            (
                'SELECT * FROM events '
                'WHERE guild_id = $1 '
                'ORDER BY start_at ASC;'
            ),
            str(ctx.guild.id)
        )
        paginator = EmbedPaginator(color=0x0000ff)
        paginator.title = 'DisCalendar - CLI'
        paginator.description = '全ての予定を表示しています'
        for event in results:
            paginator.add_line(
                event['name'],
                (
                    f'`ＩＤ`: {event["id"]}\n'
                    f'`日時`: {self.show_date(event)}\n'
                    f'`通知`: {self.show_notification(event)}\n'
                )
            )
        await paginator.paginate(ctx, 5)


def setup(bot):
    bot.add_cog(CLI(bot))
