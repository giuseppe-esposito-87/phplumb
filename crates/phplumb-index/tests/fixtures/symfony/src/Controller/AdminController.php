<?php

namespace App\Controller;

use Symfony\Component\Routing\Attribute\Route;

#[Route('/admin', name: 'admin_')]
class AdminController
{
    #[Route('/users', name: 'users')]
    public function users(): void
    {
    }
}
