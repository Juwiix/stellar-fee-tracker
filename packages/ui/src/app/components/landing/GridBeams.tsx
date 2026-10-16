'use client'

import { useEffect, useRef } from 'react'

interface Beam {
  id:        number
  axis:      'x' | 'y'       // travelling horizontally or vertically
  line:      number           // which grid line (index)
  pos:       number           // current position along the line (0–1)
  speed:     number           // units per frame
  opacity:   number           // current opacity
  length:    number           // tail length as fraction of canvas size
  color:     string
  state:     'in' | 'travel' | 'out'
  fadeAlpha: number
}

const COLORS = [
  'rgba(0, 255, 157,',   // green
  'rgba(0, 212, 255,',   // cyan
  'rgba(255, 210, 63,',  // yellow (rare)
]

const GRID_SIZE = 48 // must match globals.css backgroundSize

let beamId = 0

function randomBeam(canvasW: number, canvasH: number): Beam {
  const axis   = Math.random() > 0.5 ? 'x' : 'y'
  const cols   = Math.floor(canvasW / GRID_SIZE)
  const rows   = Math.floor(canvasH / GRID_SIZE)
  const line   = Math.floor(Math.random() * (axis === 'x' ? rows : cols))
  const speed  = 0.0008 + Math.random() * 0.0018
  const length = 0.08 + Math.random() * 0.12
  const roll   = Math.random()
  const color  = roll > 0.85
    ? COLORS[2]
    : roll > 0.4
    ? COLORS[1]
    : COLORS[0]

  return {
    id:        beamId++,
    axis,
    line,
    pos:       -length,
    speed,
    opacity:   0,
    length,
    color,
    state:     'in',
    fadeAlpha: 0,
  }
}

export function GridBeams() {
  const canvasRef = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    // The beams are pure decoration and the grid behind them is CSS, so when a
    // visitor asks for reduced motion we never start the rAF loop at all.
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return

    let animId: number
    let beams: Beam[] = []
    let lastSpawn = 0

    function resize() {
      if (!canvas) return
      canvas.width  = window.innerWidth
      canvas.height = window.innerHeight
    }

    resize()
    window.addEventListener('resize', resize)

    function spawnBeam() {
      if (!canvas) return
      beams.push(randomBeam(canvas.width, canvas.height))
    }

    // Seed a few beams immediately
    for (let i = 0; i < 4; i++) spawnBeam()

    function draw(ts: number) {
      if (!canvas || !ctx) return

      // Spawn new beam every 400–900ms randomly
      if (ts - lastSpawn > 400 + Math.random() * 500) {
        spawnBeam()
        lastSpawn = ts
      }

      ctx.clearRect(0, 0, canvas.width, canvas.height)

      beams = beams.filter(b => b.state !== 'out' || b.fadeAlpha > 0.005)

      for (const beam of beams) {
        // Advance position
        beam.pos += beam.speed

        // Fade in
        if (beam.state === 'in') {
          beam.fadeAlpha = Math.min(beam.fadeAlpha + 0.04, 1)
          if (beam.fadeAlpha >= 1) beam.state = 'travel'
        }

        // Fade out when past end
        if (beam.pos - beam.length > 1) {
          beam.state    = 'out'
          beam.fadeAlpha = Math.max(beam.fadeAlpha - 0.03, 0)
        }

        const alpha  = beam.fadeAlpha
        const W      = canvas.width
        const H      = canvas.height

        if (beam.axis === 'x') {
          // Beam travels left → right along a horizontal grid line
          const y    = (beam.line * GRID_SIZE)
          const xEnd = beam.pos * W
          const xStart = (beam.pos - beam.length) * W

          const grad = ctx.createLinearGradient(xStart, y, xEnd, y)
          grad.addColorStop(0,    `${beam.color} 0)`)
          grad.addColorStop(0.4,  `${beam.color} ${alpha * 0.15})`)
          grad.addColorStop(1,    `${beam.color} ${alpha * 0.9})`)

          ctx.beginPath()
          ctx.strokeStyle = grad
          ctx.lineWidth   = 1.5
          ctx.shadowColor = `${beam.color} ${alpha * 0.6})`
          ctx.shadowBlur  = 6
          ctx.moveTo(xStart, y)
          ctx.lineTo(xEnd,   y)
          ctx.stroke()

          // Leading dot
          ctx.beginPath()
          ctx.fillStyle = `${beam.color} ${alpha})`
          ctx.shadowBlur = 10
          ctx.arc(xEnd, y, 2, 0, Math.PI * 2)
          ctx.fill()

        } else {
          // Beam travels top → bottom along a vertical grid line
          const x    = (beam.line * GRID_SIZE)
          const yEnd = beam.pos * H
          const yStart = (beam.pos - beam.length) * H

          const grad = ctx.createLinearGradient(x, yStart, x, yEnd)
          grad.addColorStop(0,    `${beam.color} 0)`)
          grad.addColorStop(0.4,  `${beam.color} ${alpha * 0.15})`)
          grad.addColorStop(1,    `${beam.color} ${alpha * 0.9})`)

          ctx.beginPath()
          ctx.strokeStyle = grad
          ctx.lineWidth   = 1.5
          ctx.shadowColor = `${beam.color} ${alpha * 0.6})`
          ctx.shadowBlur  = 6
          ctx.moveTo(x, yStart)
          ctx.lineTo(x, yEnd)
          ctx.stroke()

          // Leading dot
          ctx.beginPath()
          ctx.fillStyle = `${beam.color} ${alpha})`
          ctx.shadowBlur = 10
          ctx.arc(x, yEnd, 2, 0, Math.PI * 2)
          ctx.fill()
        }

        ctx.shadowBlur = 0
      }

      animId = requestAnimationFrame(draw)
    }

    animId = requestAnimationFrame(draw)

    return () => {
      cancelAnimationFrame(animId)
      window.removeEventListener('resize', resize)
    }
  }, [])

  return (
    <canvas
      ref={canvasRef}
      aria-hidden="true"
      className="pointer-events-none fixed inset-0 z-[1]"
      style={{ opacity: 0.35 }}
    />
  )
}