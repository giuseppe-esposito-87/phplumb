<?php

namespace App\Handler;

use App\Message\Notify;
use App\Message\SendEmail;
use Symfony\Component\Messenger\Attribute\AsMessageHandler;

#[AsMessageHandler]
final class StaleDoc
{
    /**
     * @param Notify $message
     */
    public function __invoke(SendEmail $message): void
    {
    }
}
