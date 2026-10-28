<?php

namespace App\Handler;

use App\Message\SendSms;
use Symfony\Component\Messenger\Attribute\AsMessageHandler;

final class Explicit
{
    #[AsMessageHandler(handles: SendSms::class)]
    public function run(object $message): void
    {
    }
}
