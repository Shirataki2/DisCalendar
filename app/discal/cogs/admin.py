import asyncio
import json
import io
import time
import traceback
import textwrap
import discord
import os
from contextlib import redirect_stdout
from discord.ext import commands
from datetime import datetime
import discal
from discal.bot import Bot
from discal.logger import get_module_logger
from discal.cogs.utils.formats import plural, TabularData

logger = get_module_logger(__name__)


class Admin(commands.Cog):
    def __init__(self, bot):
        self.bot: Bot = bot
        self._last_result = None

    async def cog_check(self, ctx):
        return await self.bot.is_owner(ctx.author)

    def cleanup_code(self, content):
        """Automatically removes code blocks from the code."""
        if content.startswith('```') and content.endswith('```'):
            return '\n'.join(content.split('\n')[1:-1])
        return content.strip('` \n')

    @commands.command()
    async def load(self, ctx: commands.Context, *, module):
        try:
            self.bot.load_extension(module)
        except commands.ExtensionError as e:
            await ctx.message.add_reaction(self.bot.e_cross)
            await self.bot.send_error(ctx, f'{e.__class__.__name__}', f'{e}')
        else:
            await ctx.message.add_reaction(self.bot.e_check)

    @commands.command()
    async def unload(self, ctx: commands.Context, *, module):
        try:
            self.bot.unload_extension(module)
        except commands.ExtensionError as e:
            await ctx.message.add_reaction(self.bot.e_cross)
            await self.bot.send_error(ctx, f'{e.__class__.__name__}', f'{e}')
        else:
            await ctx.message.add_reaction(self.bot.e_check)

    @commands.group(name='reload', invoke_without_command=True)
    async def _reload(self, ctx: commands.Context, *, module):
        try:
            self.bot.reload_extension(module)
        except commands.ExtensionError as e:
            await ctx.message.add_reaction(self.bot.e_cross)
            await self.bot.send_error(ctx, f'{e.__class__.__name__}', f'{e}')
        else:
            await ctx.message.add_reaction(self.bot.e_check)

    @_reload.command(name='all')
    async def reload_all(self, ctx: commands.Context):
        self.bot.load_cogs(reload=True)
        await ctx.message.add_reaction(self.bot.e_check)

    @commands.command()
    async def sql(self, ctx: commands.Context, *, query):
        query = self.cleanup_code(query)
        is_multistatement = query.count(';') > 1
        if is_multistatement:
            strategy = self.bot.pool.execute
        else:
            strategy = self.bot.pool.fetch
        try:
            start = time.perf_counter()
            results = await strategy(query)
            dt = (time.perf_counter() - start) * 1000.0
        except Exception:
            await ctx.message.add_reaction(self.bot.e_cross)
            return await ctx.send(f'```py\n{traceback.format_exc()}\n```')
        rows = len(results)
        if is_multistatement or rows == 0:
            await ctx.message.add_reaction(self.bot.e_check)
            return await ctx.send(f'`{dt:.2f}ms: {results}`')
        headers = list(results[0].keys())
        table = TabularData()
        table.set_columns(headers)
        table.add_rows(list(r.values()) for r in results)
        render = table.render()
        fmt = f'```\n{render}\n```\n*Returned {plural(rows):row} in {dt:.2f}ms*'
        await ctx.message.add_reaction(self.bot.e_check)
        if len(fmt) > 2000:
            fp = io.BytesIO(fmt.encode('utf-8'))
            await ctx.send('Too many results...', file=discord.File(fp, 'results.txt'))
        else:
            await ctx.send(fmt)


    @commands.command(name='eval')
    async def _eval(self, ctx, *, body: str):
        """Evaluates a code"""

        env = {
            'bot': self.bot,
            'ctx': ctx,
            'channel': ctx.channel,
            'author': ctx.author,
            'guild': ctx.guild,
            'message': ctx.message,
            '_': self._last_result
        }

        env.update(globals())

        body = self.cleanup_code(body)
        stdout = io.StringIO()

        to_compile = f'async def func():\n{textwrap.indent(body, "  ")}'

        try:
            exec(to_compile, env)
        except Exception as e:
            await ctx.message.add_reaction(self.bot.e_cross)
            await self.bot.send_error(ctx, e.__class__.__name__, f'{e}')
        func = env['func']
        try:
            with redirect_stdout(stdout):
                ret = await func()
        except Exception as e:
            value = stdout.getvalue()
            await ctx.message.add_reaction(self.bot.e_cross)
            await self.bot.send_error(ctx, str(value), f'{traceback.format_exc()}')
        else:
            value = stdout.getvalue()
            try:
                await ctx.message.add_reaction(self.bot.e_check)
            except:
                pass
            if ret is None:
                if value:
                    await ctx.send(f'```py\n{value}\n```')
            else:
                self._last_result = ret
                await ctx.send(f'```py\n{value}{ret}\n```')



def setup(bot):
    bot.add_cog(Admin(bot))
