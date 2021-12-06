ARG BUILD_FROM
FROM $BUILD_FROM as builder
# FROM alpine

ENV LANG C.UTF-8

WORKDIR /opt/fireboard2mqtt

# Install requirements for add-on
RUN apk add --no-cache nodejs npm make gcc g++ py3-pip bash && \
    npm install --global yarn


COPY package.json ./
COPY ./.pnp.* ./
COPY .yarn ./.yarn
COPY .yarnrc.yml ./
COPY yarn.lock ./
RUN yarn
COPY config ./config
COPY lib ./lib
COPY ./cli.js ./
COPY ./run-ha-addon.sh ./


FROM $BUILD_FROM
ENV LANG C.UTF-8
WORKDIR /opt/fireboard2mqtt

RUN apk add --no-cache nodejs npm && \
    npm install --global yarn

COPY --from=builder /opt/fireboard2mqtt ./

CMD [ "./run-ha-addon.sh" ]
