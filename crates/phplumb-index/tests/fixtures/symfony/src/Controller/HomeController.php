<?php

namespace App\Controller;

use Symfony\Component\Routing\Attribute\Route;

class HomeController
{
    #[Route('/', name: 'home')]
    public function index(): void
    {
    }
}
