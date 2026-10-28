<?php

namespace Symfony\Component\Messenger\Attribute;

#[\Attribute(\Attribute::TARGET_CLASS | \Attribute::TARGET_METHOD | \Attribute::IS_REPEATABLE)]
class AsMessageHandler
{
    public function __construct(
        public ?string $bus = null,
        public ?string $fromTransport = null,
        public ?string $handles = null,
        public ?string $method = null,
        public int $priority = 0,
    ) {
    }
}
