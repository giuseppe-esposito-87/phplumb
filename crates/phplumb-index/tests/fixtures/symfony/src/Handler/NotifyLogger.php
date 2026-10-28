<?php

namespace App\Handler;

use App\Message\Notify;
use Symfony\Component\Messenger\Attribute\AsMessageHandler;

#[AsMessageHandler(method: 'handle')]
final class NotifyLogger
{
    public function __invoke(Notify $notification): void
    {
    }

    public function handle(Notify $notification): void
    {
    }
}
