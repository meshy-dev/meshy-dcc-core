FROM python:3.14-slim

WORKDIR /app

RUN pip install --no-cache-dir meshy-dcc-core

COPY server.py .
COPY skills/ ./skills/

ENV MESHY_DCC_HOST=0.0.0.0
ENV MESHY_DCC_PORT=8765

EXPOSE 8765

CMD ["python", "server.py"]
