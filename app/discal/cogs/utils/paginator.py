import discord
from discord.ext import commands

import asyncio

from copy import deepcopy
from math import ceil
from discal.logger import get_module_logger

from contextlib import suppress
from datetime import datetime

logger = get_module_logger(__name__)

class EmbedPaginator(commands.Paginator):
    def __init__(self, title="", description="", color=None, image="", thumb="", author="", author_link="", icon="", footer="Page $p / $P"):
        self._set_template(title, description, color, image,
                           thumb, author, author_link, icon, footer)
        self.content = {}
        self.rows = []

    @property
    def title(self):
        self.page_template["title"]
    
    @title.setter
    def title(self, title):
        self.page_template["title"] = title

    @property
    def description(self):
        self.page_template["description"]
    
    @description.setter
    def description(self, description):
        self.page_template["description"] = description

    @property
    def color(self):
        self.page_template["color"]
    
    @color.setter
    def color(self, color):
        self.page_template["color"] = color

    @property
    def footer(self):
        self.page_template["footer"]
    
    @footer.setter
    def footer(self, footer):
        self.page_template["footer"] = footer

    def _set_template(self, title="", description="", color=None, image="", thumb="", author="", author_link="", icon="", footer=""):
        self.page_template = {
            "title": title,
            "description": description,
            "color": color,
            "image": image,
            "thumb": thumb,
            "author": {
                "name": author,
                "url": author_link,
                "icon": icon
            },
            "footer": footer
        }

    def add_line(self, k, v, inline=False):
        self.rows.append({ 'name': k, 'value': v, 'inline': inline})
    
    def render(self, page, row_per_page):
        rows = self.rows[page*row_per_page:(page+1)*row_per_page]
        c = deepcopy(self.page_template)
        for k in c.keys():
            if isinstance(c[k], str):
                c[k] = c[k].replace(
                    '$p', str(page+1)
                ).replace(
                    '$P', str(ceil(len(self.rows) / row_per_page))
                )
            elif isinstance(c[k], dict):
                for l in c[k].keys():
                    if isinstance(c[k][l], str):
                        c[k][l] = c[k][l].replace(
                            '$p', str(page+1)
                        ).replace(
                            '$P', str(ceil(len(self.rows) / row_per_page))
                        )
        embed = discord.Embed()
        embed.title = c["title"]
        embed.description = c["description"]
        embed.timestamp = datetime.utcnow()
        if c["color"]:
            embed.color = c["color"]
        if c["image"] != "":
            embed.set_image(url=c["image"])
        if (name := c["author"]["name"]) != "":
            aurl = c["author"]["url"] if c["author"]["url"] != "" else discord.embeds.EmptyEmbed
            icon = c["author"]["icon"] if c["author"]["icon"] != "" else discord.embeds.EmptyEmbed
            embed.set_author(name=name, url=aurl, icon_url=icon)
        if (footer := c["footer"]) != "":
            embed.set_footer(text=footer)
        for row in rows:
            embed.add_field(**row)
        return embed

    async def paginate(self, ctx, row_per_page=5):
        curr = 0
        if len(self.rows) <= row_per_page:
            return await ctx.send(embed=self.render(curr, row_per_page))
        message = await ctx.send(embed=self.render(curr, row_per_page))
        emojis = ("\u23EE", "\u2B05", "🇽", "\u27A1", "\u23ED")
        for emoji in emojis:
            await message.add_reaction(emoji)

        def check_reaction(r: discord.Reaction, u: discord.Member):
            return all([
                r.message.id == message.id,
                str(r.emoji) in emojis,
                u.id != ctx.bot.user.id,
                ctx.author == u
            ])
        is_dm = False
        while True:
            try:
                reaction, user = await ctx.bot.wait_for("reaction_add", timeout=180, check=check_reaction)
            except asyncio.TimeoutError:
                with suppress(discord.Forbidden):
                    for emoji in emojis:
                        await message.clear_reaction(emoji)
            try:
                await message.remove_reaction(reaction.emoji, user)
            except:
                if not is_dm:
                    is_dm = True
                    await ctx.send("ページ変更するにはリアクションを解除してもう一度押してください")
            if reaction.emoji == emojis[0]:  # 最初のページへ
                curr = 0
                await message.edit(embed=self.render(curr, row_per_page))
            if reaction.emoji == emojis[1]:  # 前のページへ
                curr = max(0, curr - 1)
                await message.edit(embed=self.render(curr, row_per_page))
            if reaction.emoji == emojis[2]:  # 削除
                return await message.delete()
            if reaction.emoji == emojis[3]:  # 次のページへ
                curr = min(len(self.content) - 1, curr + 1)
                await message.edit(embed=self.render(curr, row_per_page))
            if reaction.emoji == emojis[4]:  # 最後のページへ
                curr = len(self.content) - 1
                await message.edit(embed=self.render(curr, row_per_page))
