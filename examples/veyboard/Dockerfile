FROM node:22-alpine

WORKDIR /app

# The compiler that builds the project.
RUN npm install -g vey

COPY . .

# The server is built into one file, so that the container only runs it.
RUN vey build --node -o server.cjs

EXPOSE 4000
CMD ["node", "--enable-source-maps", "server.cjs"]
