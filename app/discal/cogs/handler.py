import asyncio
import json
import discord
from discord.ext import commands, tasks
from discal.bot import Bot
from datetime import datetime, timedelta
from discal.logger import get_module_logger


logger = get_module_logger(__name__)


class Handler(commands.Cog):
    def __init__(self, bot):
        self.bot: Bot = bot
        self.postloop.start()

    def cog_unload(self):
        self.postloop.cancel()

    async def post_subtask(self, record):
        setting = await self.bot.pool.fetchrow(
            (
                'SELECT * FROM event_settings WHERE '
                'guild_id = $1;'
            ),
            record['guild_id']
        )
        if setting is None:
            return
        guild = self.bot.get_guild(int(setting['guild_id']))
        channel = guild.get_channel(int(setting['channel_id']))
        notifications = [
            json.loads(notification)
            for notification in record['notifications']
        ]
        notifications.append({'key': -1, 'num': 0, 'type': '分前'})
        for notification in notifications:
            minutes = int(notification['num'])
            if notification['type'] == '時間前':
                minutes *= 60
            elif notification['type'] == '日前':
                minutes *= 24 * 60
            elif notification['type'] == '週間前':
                minutes *= 7 * 24 * 60
            start = record['start_at']
            end = record['end_at']
            if record['is_all_day']:
                start = datetime(
                    start.year, start.month, start.day,
                    0, 0, 0, 0
                )
                end = datetime(
                    end.year, end.month, end.day,
                    0, 0, 0, 0
                )
            now_minus_1 = datetime.now() + timedelta(hours=9, minutes=minutes - 1)
            now = datetime.now() + timedelta(hours=9, minutes=minutes)
            if start >= now_minus_1 and start < now:
                embed = discord.Embed(color=int(record['color'][1:], 16))
                embed.title = record['name']
                if record['description']:
                    embed.description = record['description']
                if notification['key'] == -1:
                    embed.set_author(name='以下の予定が開催されます')
                else:
                    prefix = f'{notification["num"]}{notification["type"][:-1]}後に'
                    embed.set_author(name=f'{prefix}以下の予定が開催されます')
                if record['is_all_day']:
                    if start == end:
                        v = f'{start.strftime("%Y/%m/%d")}'
                    else:
                        v = f'{start.strftime("%Y/%m/%d")} - {end.strftime("%Y/%m/%d")}'
                else:
                    start_date = datetime(start.year, start.month, start.day)
                    end_date = datetime(end.year, end.month, end.day)
                    if start_date == end_date:
                        v = f'{start.strftime("%Y/%m/%d %H:%M")} - {end.strftime("%H:%M")}'
                    else:
                        v = f'{start.strftime("%Y/%m/%d %H:%M")} - {end.strftime("%Y/%m/%d %H:%M")}'
                embed.add_field(name='日時', value=v, inline=False)
                logger.info(f'Send Notification: {record}')
                await channel.send(embed=embed)

    @tasks.loop(minutes=1)
    async def postloop(self):
        records = await self.bot.pool.fetch(
            (
                'SELECT * FROM events WHERE '
                'start_at >= $1;'
            ),
            datetime.now()
        )
        asyncio.gather(*[
            self.post_subtask(record)
            for record in records
        ], loop=self.bot.loop)

    @postloop.before_loop
    async def wait_ready(self):
        logger.info('waiting...')
        await self.bot.wait_until_ready()


def setup(bot):
    bot.add_cog(Handler(bot))
