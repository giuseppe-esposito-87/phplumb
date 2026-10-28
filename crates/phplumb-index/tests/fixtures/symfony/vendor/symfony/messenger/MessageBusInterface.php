<?php

namespace Symfony\Component\Messenger;

interface MessageBusInterface
{
    public function dispatch(object $message, array $stamps = []): Envelope;
}
