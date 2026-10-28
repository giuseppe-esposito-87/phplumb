<?php

namespace App\Controller;

use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;

class LinkController extends AbstractController
{
    public function links(): void
    {
        $this->redirectToRoute('demo.attr');
        $this->redirectToRoute('symfony_home');
    }
}
