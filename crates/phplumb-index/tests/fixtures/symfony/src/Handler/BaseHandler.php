<?php

namespace App\Handler;

use App\Message\Unhandled;
use Symfony\Component\Messenger\Attribute\AsMessageHandler;

#[AsMessageHandler]
abstract class BaseHandler
{
    public function __invoke(Unhandled $message): void
    {
    }
}
