<?php

namespace App\Handler;

use App\Message\SendEmail;
use App\Message\SendSms;
use Symfony\Component\Messenger\Attribute\AsMessageHandler;

final class SmsHandlers
{
    #[AsMessageHandler]
    public function onSms(SendSms|SendEmail $message): void
    {
    }
}
