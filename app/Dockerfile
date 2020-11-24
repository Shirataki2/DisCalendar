FROM python:3.8 as builder
WORKDIR /workspace
COPY ./Pipfile ./Pipfile.lock /workspace/
RUN pip install pipenv && \
    pipenv install --system

FROM python:3.8-slim
WORKDIR /workspace/konoha
RUN apt update -y && \
    apt upgrade -y && \
    apt install -y nodejs npm curl && \
    npm install -g n && \
    n stable && \
    apt purge -y nodejs npm && \
    apt install -y ffmpeg && \
    apt autoremove -y
RUN npm install -g nodemon
ENV PYTHONBUFFERED=1
ENV PYTHONPYCACHEPREFIX=/cache

COPY --from=builder /usr/local/lib/python3.8/site-packages /usr/local/lib/python3.8/site-packages
