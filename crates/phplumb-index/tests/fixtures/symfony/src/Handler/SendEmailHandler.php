<?php

namespace App\Handler;

use App\Message\SendEmail;
use Symfony\Component\Messenger\Attribute\AsMessageHandler;

#[AsMessageHandler]
final class SendEmailHandler
{
    public function __invoke(SendEmail $message): void
    {
    }
}
