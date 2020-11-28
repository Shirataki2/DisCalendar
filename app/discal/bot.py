import discord
from discord.ext import commands
import os
import glob
import asyncpg
from discal import __version__
from discal.logger import get_module_logger


logger = get_module_logger(__name__)


ENABLED_COGS = (
    "discal.cogs.miscs",
    "discal.cogs.register",
    "discal.cogs.handler",
)


class Bot(commands.AutoShardedBot):
    def __init__(self, *args, **kwargs):
        super(Bot, self).__init__(*args, **kwargs)
        self.pool = self.loop.run_until_complete(self.initialize_db())
        self.load_cogs()

    def load_cogs(self, reload=False):
        logger.info(f"{'Rel' if reload else 'L'}oading Cogs...")
        for cog in ENABLED_COGS:
            try:
                if reload:
                    self.reload_extension(cog)
                else:
                    self.load_extension(cog)
                logger.info(f'\t{cog} ... OK')
            except Exception as e:
                logger.error(f'\t{cog} ... Failed')
                raise e

    async def initialize_db(self):
        logger.info("Creating database pool...")
        return await asyncpg.create_pool(
            host="db",
            user=os.environ["POSTGRES_USER"],
            password=os.environ["POSTGRES_PASSWORD"],
            database=os.environ["POSTGRES_DB"],
            loop=self.loop,
        )
    
    async def on_ready(self):
        logger.info("Ready.")
        logger.info("Bot Name : %s", self.user)
        logger.info("Bot  ID  : %s", self.user.id)
        logger.info("Version  : %s", __version__)
        await self.change_presence(status=discord.Status.online)
