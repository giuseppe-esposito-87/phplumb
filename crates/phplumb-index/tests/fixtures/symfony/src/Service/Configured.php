<?php

namespace App\Service;

use Symfony\Component\DependencyInjection\Attribute\Autowire;

final class Configured
{
    public function __construct(
        #[Autowire(param: 'app.name')] private string $name,
        #[Autowire('%app.limit%')] private int $limit,
        #[Autowire('%kernel.project_dir%/x')] private string $dir,
        #[Autowire(env: 'APP_X')] private string $x,
    ) {
    }
}
