<?php

namespace App\Controller;

use Symfony\Component\Routing\Attribute\Route;

class HomeController
{
    #[Route('/home', name: 'symfony_home')]
    public function home(): void
    {
    }
}
